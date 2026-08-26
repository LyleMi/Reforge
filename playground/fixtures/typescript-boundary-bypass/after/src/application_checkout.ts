import { submitPayment } from "./payment_gateway.ts";

export function charge(paymentRequest: string): string {
  return submitPayment(paymentRequest);
}
