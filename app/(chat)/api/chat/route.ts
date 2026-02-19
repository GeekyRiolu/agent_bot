import { geolocation } from "@vercel/functions";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  generateId,
  stepCountIs,
  streamText,
} from "ai";
import { after } from "next/server";
import { createResumableStreamContext } from "resumable-stream";
import { auth, type UserType } from "@/app/(auth)/auth";
import { entitlementsByUserType } from "@/lib/ai/entitlements";
import { type RequestHints, systemPrompt } from "@/lib/ai/prompts";
import { getLanguageModel } from "@/lib/ai/providers";
import { createDocument } from "@/lib/ai/tools/create-document";
import { getWeather } from "@/lib/ai/tools/get-weather";
import { requestSuggestions } from "@/lib/ai/tools/request-suggestions";
import { updateDocument } from "@/lib/ai/tools/update-document";
import { isProductionEnvironment } from "@/lib/constants";
import {
  createStreamId,
  deleteChatById,
  getChatById,
  getMessageCountByUserId,
  getMessagesByChatId,
  saveChat,
  saveMessages,
  updateChatTitleById,
  updateMessage,
} from "@/lib/db/queries";
import type { DBMessage } from "@/lib/db/schema";
import { ChatSDKError } from "@/lib/errors";
import type { ChatMessage } from "@/lib/types";
import { convertToUIMessages, generateUUID } from "@/lib/utils";
import { generateTitleFromUserMessage } from "../../actions";
import { type PostRequestBody, postRequestBodySchema } from "./schema";

export const maxDuration = 60;
const RUST_REQUEST_TIMEOUT_MS = 25_000;
const RUST_MAX_ATTEMPTS = 2;

type RustChatRequest = {
  chat_id?: string;
  tenant_id?: string;
  user_id?: string;
  messages: Array<{
    role: string;
    content: string;
  }>;
};

type RustApiResponse = {
  success: boolean;
  data?: unknown;
  error?: string | null;
  timestamp?: string;
};

function getStreamContext() {
  try {
    return createResumableStreamContext({ waitUntil: after });
  } catch (_) {
    return null;
  }
}

export { getStreamContext };

function toRustChatMessages(messages: ChatMessage[]): RustChatRequest["messages"] {
  const normalized = messages
    .map((message) => {
      const content = message.parts
        .filter((part) => part.type === "text")
        .map((part) => part.text)
        .join("\n")
        .trim();

      return {
        role: message.role,
        content,
      };
    })
    .filter((message) => message.content.length > 0);

  // Keep request payload bounded for Cloud Run stability.
  return normalized.slice(-24);
}

function extractRustAnswer(data: unknown): string {
  if (typeof data === "string") {
    return data;
  }

  if (data && typeof data === "object") {
    const payload = data as Record<string, unknown>;

    if (typeof payload.answer === "string") {
      return payload.answer;
    }

    if (typeof payload.message === "string") {
      return payload.message;
    }

    if (typeof payload.summary === "string") {
      return payload.summary;
    }

    return JSON.stringify(payload, null, 2);
  }

  return "No response returned by Rust service.";
}

async function callRustChatService({
  rustApiUrl,
  messages,
  chatId,
  userId,
}: {
  rustApiUrl: string;
  messages: ChatMessage[];
  chatId: string;
  userId: string;
}): Promise<string> {
  const rustMessages = toRustChatMessages(messages);

  if (rustMessages.length === 0) {
    throw new ChatSDKError(
      "bad_request:api",
      "No text messages available to send to Rust service."
    );
  }

  const body = JSON.stringify({
    chat_id: chatId,
    tenant_id: process.env.RUST_TENANT_ID ?? "default",
    user_id: userId,
    messages: rustMessages,
  } satisfies RustChatRequest);

  let lastError: unknown = null;

  for (let attempt = 1; attempt <= RUST_MAX_ATTEMPTS; attempt++) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), RUST_REQUEST_TIMEOUT_MS);

    try {
      const response = await fetch(`${rustApiUrl.replace(/\/$/, "")}/api/chat`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body,
        signal: controller.signal,
      });

      if (!response.ok) {
        const responseText = await response.text();
        throw new ChatSDKError(
          "bad_request:api",
          `Rust service error (${response.status}): ${responseText}`
        );
      }

      const payload = (await response.json()) as RustApiResponse;

      if (!payload.success) {
        throw new ChatSDKError(
          "bad_request:api",
          payload.error ?? "Rust service returned an error"
        );
      }

      return extractRustAnswer(payload.data);
    } catch (error) {
      lastError = error;
      if (attempt === RUST_MAX_ATTEMPTS) {
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 400));
    } finally {
      clearTimeout(timeout);
    }
  }

  if (lastError instanceof Error && lastError.name === "AbortError") {
    throw new ChatSDKError(
      "offline:chat",
      "Rust service timed out. Please retry."
    );
  }

  if (lastError instanceof ChatSDKError) {
    throw lastError;
  }

  throw new ChatSDKError("offline:chat", "Rust service request failed.");
}

export async function POST(request: Request) {
  let requestBody: PostRequestBody;

  try {
    const json = await request.json();
    requestBody = postRequestBodySchema.parse(json);
  } catch (_) {
    return new ChatSDKError("bad_request:api").toResponse();
  }

  try {
    const { id, message, messages, selectedChatModel, selectedVisibilityType } =
      requestBody;

    const session = await auth();

    if (!session?.user) {
      return new ChatSDKError("unauthorized:chat").toResponse();
    }

    const userType: UserType = session.user.type;

    const messageCount = await getMessageCountByUserId({
      id: session.user.id,
      differenceInHours: 24,
    });

    if (messageCount > entitlementsByUserType[userType].maxMessagesPerDay) {
      return new ChatSDKError("rate_limit:chat").toResponse();
    }

    const isToolApprovalFlow = Boolean(messages);

    const chat = await getChatById({ id });
    let messagesFromDb: DBMessage[] = [];
    let titlePromise: Promise<string> | null = null;

    if (chat) {
      if (chat.userId !== session.user.id) {
        return new ChatSDKError("forbidden:chat").toResponse();
      }
      if (!isToolApprovalFlow) {
        messagesFromDb = await getMessagesByChatId({ id });
      }
    } else if (message?.role === "user") {
      await saveChat({
        id,
        userId: session.user.id,
        title: "New chat",
        visibility: selectedVisibilityType,
      });
      titlePromise = generateTitleFromUserMessage({ message });
    }

    const uiMessages = isToolApprovalFlow
      ? (messages as ChatMessage[])
      : [...convertToUIMessages(messagesFromDb), message as ChatMessage];

    const { longitude, latitude, city, country } = geolocation(request);

    const requestHints: RequestHints = {
      longitude,
      latitude,
      city,
      country,
    };

    if (message?.role === "user") {
      await saveMessages({
        messages: [
          {
            chatId: id,
            id: message.id,
            role: "user",
            parts: message.parts,
            attachments: [],
            createdAt: new Date(),
          },
        ],
      });
    }

    const isReasoningModel =
      selectedChatModel.includes("reasoning") ||
      selectedChatModel.includes("thinking");

    const modelMessages = await convertToModelMessages(uiMessages);
    const rustApiUrl = process.env.RUST_API_URL;

    const stream = createUIMessageStream({
      originalMessages: isToolApprovalFlow ? uiMessages : undefined,
      execute: async ({ writer: dataStream }) => {
        if (rustApiUrl) {
          const textId = generateUUID();
          dataStream.write({
            type: "text-start",
            id: textId,
          });
          dataStream.write({
            type: "text-delta",
            id: textId,
            delta: "Working on it...\n\n",
          });

          const rustAnswer = await callRustChatService({
            rustApiUrl,
            messages: uiMessages,
            chatId: id,
            userId: session.user.id,
          });

          dataStream.write({
            type: "text-delta",
            id: textId,
            delta: rustAnswer,
          });
          dataStream.write({
            type: "text-end",
            id: textId,
          });

          if (titlePromise) {
            const title = await titlePromise;
            dataStream.write({ type: "data-chat-title", data: title });
            updateChatTitleById({ chatId: id, title });
          }

          return;
        }

        const result = streamText({
          model: getLanguageModel(selectedChatModel),
          system: systemPrompt({ selectedChatModel, requestHints }),
          messages: modelMessages,
          stopWhen: stepCountIs(5),
          experimental_activeTools: isReasoningModel
            ? []
            : [
                "getWeather",
                "createDocument",
                "updateDocument",
                "requestSuggestions",
              ],
          providerOptions: isReasoningModel
            ? {
                anthropic: {
                  thinking: { type: "enabled", budgetTokens: 10_000 },
                },
              }
            : undefined,
          tools: {
            getWeather,
            createDocument: createDocument({ session, dataStream }),
            updateDocument: updateDocument({ session, dataStream }),
            requestSuggestions: requestSuggestions({ session, dataStream }),
          },
          experimental_telemetry: {
            isEnabled: isProductionEnvironment,
            functionId: "stream-text",
          },
        });

        dataStream.merge(result.toUIMessageStream({ sendReasoning: true }));

        if (titlePromise) {
          const title = await titlePromise;
          dataStream.write({ type: "data-chat-title", data: title });
          updateChatTitleById({ chatId: id, title });
        }
      },
      generateId: generateUUID,
      onFinish: async ({ messages: finishedMessages }) => {
        if (isToolApprovalFlow) {
          for (const finishedMsg of finishedMessages) {
            const existingMsg = uiMessages.find((m) => m.id === finishedMsg.id);
            if (existingMsg) {
              await updateMessage({
                id: finishedMsg.id,
                parts: finishedMsg.parts,
              });
            } else {
              await saveMessages({
                messages: [
                  {
                    id: finishedMsg.id,
                    role: finishedMsg.role,
                    parts: finishedMsg.parts,
                    createdAt: new Date(),
                    attachments: [],
                    chatId: id,
                  },
                ],
              });
            }
          }
        } else if (finishedMessages.length > 0) {
          await saveMessages({
            messages: finishedMessages.map((currentMessage) => ({
              id: currentMessage.id,
              role: currentMessage.role,
              parts: currentMessage.parts,
              createdAt: new Date(),
              attachments: [],
              chatId: id,
            })),
          });
        }
      },
      onError: () => "Oops, an error occurred!",
    });

    return createUIMessageStreamResponse({
      stream,
      async consumeSseStream({ stream: sseStream }) {
        if (!process.env.REDIS_URL) {
          return;
        }
        try {
          const streamContext = getStreamContext();
          if (streamContext) {
            const streamId = generateId();
            await createStreamId({ streamId, chatId: id });
            await streamContext.createNewResumableStream(
              streamId,
              () => sseStream
            );
          }
        } catch (_) {
          // ignore redis errors
        }
      },
    });
  } catch (error) {
    const vercelId = request.headers.get("x-vercel-id");

    if (error instanceof ChatSDKError) {
      return error.toResponse();
    }

    if (
      error instanceof Error &&
      error.message?.includes(
        "AI Gateway requires a valid credit card on file to service requests"
      )
    ) {
      return new ChatSDKError("bad_request:activate_gateway").toResponse();
    }

    console.error("Unhandled error in chat API:", error, { vercelId });
    return new ChatSDKError("offline:chat").toResponse();
  }
}

export async function DELETE(request: Request) {
  const { searchParams } = new URL(request.url);
  const id = searchParams.get("id");

  if (!id) {
    return new ChatSDKError("bad_request:api").toResponse();
  }

  const session = await auth();

  if (!session?.user) {
    return new ChatSDKError("unauthorized:chat").toResponse();
  }

  const chat = await getChatById({ id });

  if (chat?.userId !== session.user.id) {
    return new ChatSDKError("forbidden:chat").toResponse();
  }

  const deletedChat = await deleteChatById({ id });

  return Response.json(deletedChat, { status: 200 });
}
