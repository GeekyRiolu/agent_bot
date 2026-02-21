"use client";

import { signIn, useSession } from "next-auth/react";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { GoogleAuthProvider, signInWithPopup } from "firebase/auth";
import { toast } from "@/components/toast";
import { Button } from "@/components/ui/button";
import { getFirebaseClientAuth } from "@/lib/auth/firebase-client";

export function GoogleSignInButton() {
  const router = useRouter();
  const { update: updateSession } = useSession();
  const [isLoading, setIsLoading] = useState(false);

  const handleGoogleSignIn = async () => {
    try {
      setIsLoading(true);

      const auth = getFirebaseClientAuth();
      const provider = new GoogleAuthProvider();
      const credential = await signInWithPopup(auth, provider);
      const idToken = await credential.user.getIdToken();

      const result = await signIn("firebase", {
        idToken,
        redirect: false,
      });

      if (result?.error) {
        throw new Error(result.error);
      }

      // Refresh client-side session so useSession() picks up the new user
      await updateSession();
      router.refresh();
      router.push("/");
    } catch (error) {
      const message =
        error instanceof Error && error.message
          ? error.message
          : "Unknown Firebase auth error";
      console.error("Google sign-in failed:", error);
      toast({
        type: "error",
        description: `Google sign-in failed: ${message}`,
      });
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Button
      className="w-full"
      disabled={isLoading}
      onClick={handleGoogleSignIn}
      type="button"
      variant="outline"
    >
      {isLoading ? "Signing in..." : "Continue with Google"}
    </Button>
  );
}
