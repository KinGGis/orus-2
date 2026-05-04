/**
 * Supabase client for accessing Orus data
 * This provides direct access to existing Orus tables in Supabase
 */
import { createClient } from "@supabase/supabase-js";
import type { OrusDatabase } from "./types";

// Orus Supabase configuration
const SUPABASE_URL = "https://bujfwfqmfsgaibnmuvml.supabase.co";
const SUPABASE_ANON_KEY =
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImJ1amZ3ZnFtZnNnYWlibm11dm1sIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NDUyNzUyMzYsImV4cCI6MjA2MDg1MTIzNn0.5dzs-01w-2tm21tfDmJIHzHFAnjBEKCapZfLSBLUiUo";

export const orusSupabase = createClient<OrusDatabase>(SUPABASE_URL, SUPABASE_ANON_KEY, {
  auth: {
    storage: typeof window !== "undefined" ? localStorage : undefined,
    persistSession: true,
    autoRefreshToken: true,
    storageKey: "orus-auth-token",
  },
});

// Re-export for convenience
export { SUPABASE_URL };
