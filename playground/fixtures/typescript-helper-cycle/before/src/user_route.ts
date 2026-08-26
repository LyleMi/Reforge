import { userSummary } from "./user_service";

export function showUser(name: string): string {
  return userSummary(name);
}

export function requestLocale(): string {
  return "en";
}
