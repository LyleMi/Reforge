import { priceCart } from "./pricing";

export function checkout(items: string[]): number {
  return priceCart(items);
}
