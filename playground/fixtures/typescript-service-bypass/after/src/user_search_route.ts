import { queryUser } from "./database.ts";

export function searchUser(userId: string): string {
  return queryUser(userId);
}
