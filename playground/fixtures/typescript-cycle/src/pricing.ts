import { promotionFor } from "./promotions";

export function priceCart(items: string[]): number {
  return items.length * 20 - promotionFor(items.length);
}
