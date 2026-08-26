import { findUser } from "./user_service.ts";

export function getUser(userId: string): string {
  return findUser(userId);
}
