import { checkout } from "./checkout";

export function promotionFor(itemCount: number): number {
  return itemCount > 3 ? checkout([]) : 0;
}
