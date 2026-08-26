import { priceCart } from "./pricing";

export function checkout(items: string[]): number {
  return priceCart(items);
}

export function hasCompletedOrder(customerId: string): boolean {
  return customerId.startsWith("returning-");
}
