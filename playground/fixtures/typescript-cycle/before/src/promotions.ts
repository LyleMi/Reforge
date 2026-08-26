export function promotionFor(customerId: string, itemCount: number): number {
  return itemCount > 3 ? 5 : 0;
}
