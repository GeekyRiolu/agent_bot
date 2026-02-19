"use client";

import { initializeApp, getApps, getApp } from "firebase/app";
import { getAuth } from "firebase/auth";

const firebaseConfig = {
  apiKey: process.env.NEXT_PUBLIC_FIREBASE_API_KEY,
  authDomain: process.env.NEXT_PUBLIC_FIREBASE_AUTH_DOMAIN,
  projectId: process.env.NEXT_PUBLIC_FIREBASE_PROJECT_ID,
  appId: process.env.NEXT_PUBLIC_FIREBASE_APP_ID,
};

function assertFirebaseClientConfig() {
  const requiredKeys: Array<keyof typeof firebaseConfig> = [
    "apiKey",
    "authDomain",
    "projectId",
    "appId",
  ];

  for (const key of requiredKeys) {
    if (!firebaseConfig[key]) {
      throw new Error(
        `Missing ${key} for Firebase client config. Check NEXT_PUBLIC_FIREBASE_* env vars.`
      );
    }
  }
}

export function getFirebaseClientAuth() {
  assertFirebaseClientConfig();

  const app = getApps().length > 0 ? getApp() : initializeApp(firebaseConfig);
  return getAuth(app);
}
