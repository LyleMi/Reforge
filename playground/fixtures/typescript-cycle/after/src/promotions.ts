import { hasCompletedOrder } from "./checkout";

export function promotionFor(customerId: string, itemCount: number): number {
  const firstOrderDiscount = hasCompletedOrder(customerId) ? 0 : 5;
  return itemCount > 3 ? 5 + firstOrderDiscount : firstOrderDiscount;
}
