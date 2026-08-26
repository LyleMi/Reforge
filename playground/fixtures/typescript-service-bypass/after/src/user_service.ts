import { queryUser } from "./database.ts";

export function findUser(userId: string): string {
  return queryUser(userId);
}
