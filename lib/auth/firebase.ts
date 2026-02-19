import "server-only";

type FirebaseErrorPayload = {
  error?: {
    message?: string;
  };
};

type FirebaseSignInResponse = {
  idToken: string;
  localId: string;
  email?: string;
};

type FirebaseLookupResponse = {
  users?: Array<{
    localId: string;
    email?: string;
    emailVerified?: boolean;
  }>;
};

export class FirebaseAuthError extends Error {
  code: string;

  constructor(code: string, message?: string) {
    super(message ?? code);
    this.name = "FirebaseAuthError";
    this.code = code;
  }
}

function getFirebaseApiKey() {
  const apiKey = process.env.FIREBASE_WEB_API_KEY;

  if (!apiKey) {
    throw new FirebaseAuthError(
      "MISSING_FIREBASE_WEB_API_KEY",
      "FIREBASE_WEB_API_KEY is not configured"
    );
  }

  return apiKey;
}

async function firebaseAuthRequest<T>(
  endpoint: string,
  payload: Record<string, unknown>
): Promise<T> {
  const apiKey = getFirebaseApiKey();

  const response = await fetch(
    `https://identitytoolkit.googleapis.com/v1/${endpoint}?key=${apiKey}`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    }
  );

  const body = (await response.json()) as FirebaseErrorPayload & T;

  if (!response.ok) {
    const code = body.error?.message ?? "FIREBASE_AUTH_ERROR";
    throw new FirebaseAuthError(code, code);
  }

  return body;
}

export async function signUpWithEmailPassword(
  email: string,
  password: string
): Promise<FirebaseSignInResponse> {
  return firebaseAuthRequest<FirebaseSignInResponse>("accounts:signUp", {
    email,
    password,
    returnSecureToken: true,
  });
}

export async function signInWithEmailPassword(
  email: string,
  password: string
): Promise<FirebaseSignInResponse> {
  return firebaseAuthRequest<FirebaseSignInResponse>(
    "accounts:signInWithPassword",
    {
      email,
      password,
      returnSecureToken: true,
    }
  );
}

export async function verifyFirebaseIdToken(idToken: string): Promise<{
  localId: string;
  email?: string;
  emailVerified?: boolean;
}> {
  const result = await firebaseAuthRequest<FirebaseLookupResponse>(
    "accounts:lookup",
    {
      idToken,
    }
  );

  const user = result.users?.[0];

  if (!user?.localId) {
    throw new FirebaseAuthError("INVALID_ID_TOKEN", "Invalid Firebase ID token");
  }

  return user;
}
