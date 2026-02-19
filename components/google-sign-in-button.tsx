"use client";

import { signIn } from "next-auth/react";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { GoogleAuthProvider, signInWithPopup } from "firebase/auth";
import { toast } from "@/components/toast";
import { Button } from "@/components/ui/button";
import { getFirebaseClientAuth } from "@/lib/auth/firebase-client";

export function GoogleSignInButton() {
  const router = useRouter();
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

      router.refresh();
      router.push("/");
    } catch (_error) {
      toast({
        type: "error",
        description: "Google sign-in failed. Check Firebase config and try again.",
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
