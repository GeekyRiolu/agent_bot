"use server";

import { z } from "zod";
import {
  FirebaseAuthError,
  signInWithEmailPassword,
  signUpWithEmailPassword,
} from "@/lib/auth/firebase";

import { signIn } from "./auth";

const authFormSchema = z.object({
  email: z.string().email(),
  password: z.string().min(6),
});

export type LoginActionState = {
  status: "idle" | "in_progress" | "success" | "failed" | "invalid_data";
};

export const login = async (
  _: LoginActionState,
  formData: FormData
): Promise<LoginActionState> => {
  try {
    const validatedData = authFormSchema.parse({
      email: formData.get("email"),
      password: formData.get("password"),
    });

    const firebaseSession = await signInWithEmailPassword(
      validatedData.email,
      validatedData.password
    );

    await signIn("firebase", {
      idToken: firebaseSession.idToken,
      redirect: false,
    });

    return { status: "success" };
  } catch (error) {
    if (error instanceof z.ZodError) {
      return { status: "invalid_data" };
    }

    if (error instanceof FirebaseAuthError) {
      return { status: "failed" };
    }

    return { status: "failed" };
  }
};

export type RegisterActionState = {
  status:
    | "idle"
    | "in_progress"
    | "success"
    | "failed"
    | "user_exists"
    | "invalid_data";
};

export const register = async (
  _: RegisterActionState,
  formData: FormData
): Promise<RegisterActionState> => {
  try {
    const validatedData = authFormSchema.parse({
      email: formData.get("email"),
      password: formData.get("password"),
    });

    const firebaseSession = await signUpWithEmailPassword(
      validatedData.email,
      validatedData.password
    );

    await signIn("firebase", {
      idToken: firebaseSession.idToken,
      redirect: false,
    });

    return { status: "success" };
  } catch (error) {
    if (error instanceof z.ZodError) {
      return { status: "invalid_data" };
    }

    if (error instanceof FirebaseAuthError && error.code === "EMAIL_EXISTS") {
      return { status: "user_exists" };
    }

    return { status: "failed" };
  }
};
