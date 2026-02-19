ALTER TABLE "User" ADD COLUMN IF NOT EXISTS "firebaseUid" varchar(128);
--> statement-breakpoint
CREATE UNIQUE INDEX IF NOT EXISTS "User_firebaseUid_key" ON "User" USING btree ("firebaseUid");
