"use client";

import { useRouter } from "next/navigation";
import { useSession } from "next-auth/react";
import { useEffect, useState } from "react";
import { GoogleSignInButton } from "@/components/google-sign-in-button";
import { SparklesIcon } from "@/components/icons";
import { Button } from "@/components/ui/button";
import { guestRegex } from "@/lib/constants";

const DISMISSED_KEY = "welcome-modal-dismissed";

export function WelcomeModal() {
  const { data: session, status } = useSession();
  const router = useRouter();
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (status === "loading") {
      return;
    }

    const isGuest = guestRegex.test(session?.user?.email ?? "");
    const dismissed = localStorage.getItem(DISMISSED_KEY);

    if (isGuest && !dismissed) {
      // Tiny delay so the page content renders first, then modal fades in
      const timer = setTimeout(() => setOpen(true), 300);
      return () => clearTimeout(timer);
    }
  }, [session, status]);

  if (!open) {
    return null;
  }

  const handleDismiss = () => {
    localStorage.setItem(DISMISSED_KEY, "true");
    setOpen(false);
  };

  const handleSignIn = () => {
    localStorage.setItem(DISMISSED_KEY, "true");
    router.push("/login");
  };

  return (
    <div className="welcome-overlay fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div
        className="welcome-card relative mx-4 flex w-full max-w-md flex-col overflow-hidden rounded-2xl border border-border/60 bg-background shadow-2xl"
        role="dialog"
        aria-label="Welcome"
        aria-modal="true"
      >
        {/* Decorative gradient header */}
        <div className="relative flex flex-col items-center gap-3 bg-gradient-to-b from-primary/8 to-transparent px-8 pt-10 pb-6">
          <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10 ring-1 ring-primary/20">
            <SparklesIcon size={28} />
          </div>
          <h2 className="font-semibold text-xl tracking-tight">
            Welcome to Samwise
          </h2>
          <p className="max-w-xs text-center text-muted-foreground text-sm leading-relaxed">
            Sign in to save your chats and access them from anywhere, or
            continue as a guest to get started right away.
          </p>
        </div>

        {/* Actions */}
        <div className="flex flex-col gap-3 px-8 pb-8">
          {/* Google Sign In */}
          <GoogleSignInButton />

          {/* Divider */}
          <div className="flex items-center gap-3">
            <div className="h-px flex-1 bg-border" />
            <span className="text-muted-foreground text-xs">or</span>
            <div className="h-px flex-1 bg-border" />
          </div>

          {/* Email Sign In */}
          <Button
            className="w-full"
            onClick={handleSignIn}
            type="button"
            variant="outline"
          >
            Sign in with Email
          </Button>

          {/* Guest Continue */}
          <Button
            className="w-full text-muted-foreground"
            onClick={handleDismiss}
            type="button"
            variant="ghost"
          >
            Continue as Guest
          </Button>
        </div>
      </div>
    </div>
  );
}
