import { requestLocale } from "./user_route";

export function formatUserName(name: string): string {
  const normalized = name.trim();
  return requestLocale() === "en" ? normalized : normalized.toLocaleUpperCase();
}
