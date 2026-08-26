import { formatUserName } from "./user_formatter";

export function userSummary(name: string): string {
  return `User: ${formatUserName(name)}`;
}
