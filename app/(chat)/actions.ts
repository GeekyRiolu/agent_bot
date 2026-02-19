"use server";

import { generateText, type UIMessage } from "ai";
import { cookies } from "next/headers";
import type { VisibilityType } from "@/components/visibility-selector";
import { titlePrompt } from "@/lib/ai/prompts";
import { getTitleModel } from "@/lib/ai/providers";
import {
  deleteMessagesByChatIdAfterTimestamp,
  getMessageById,
  updateChatVisibilityById,
} from "@/lib/db/queries";
import { getTextFromMessage } from "@/lib/utils";

export async function saveChatModelAsCookie(model: string) {
  const cookieStore = await cookies();
  cookieStore.set("chat-model", model);
}

export async function generateTitleFromUserMessage({
  message,
}: {
  message: UIMessage;
}) {
  const prompt = getTextFromMessage(message);

  const fallbackTitle = (() => {
    const cleaned = prompt
      .replace(/\s+/g, " ")
      .replace(/[^\p{L}\p{N}\s]/gu, " ")
      .trim();

    if (!cleaned) {
      return "New Conversation";
    }

    return cleaned
      .split(" ")
      .filter(Boolean)
      .slice(0, 5)
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(" ");
  })();

  if (process.env.RUST_API_URL || !process.env.AI_GATEWAY_API_KEY) {
    return fallbackTitle;
  }

  try {
    const { text } = await generateText({
      model: getTitleModel(),
      system: titlePrompt,
      prompt,
    });

    const title = text
      .replace(/^[#*"\s]+/, "")
      .replace(/["]+$/, "")
      .trim();

    return title || fallbackTitle;
  } catch (_) {
    return fallbackTitle;
  }
}

export async function deleteTrailingMessages({ id }: { id: string }) {
  const [message] = await getMessageById({ id });

  await deleteMessagesByChatIdAfterTimestamp({
    chatId: message.chatId,
    timestamp: message.createdAt,
  });
}

export async function updateChatVisibility({
  chatId,
  visibility,
}: {
  chatId: string;
  visibility: VisibilityType;
}) {
  await updateChatVisibilityById({ chatId, visibility });
}
