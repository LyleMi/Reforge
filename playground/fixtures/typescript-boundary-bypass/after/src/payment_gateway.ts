import { sendPayment } from "./transport.ts";

export function submitPayment(paymentRequest: string): string {
  return sendPayment(paymentRequest);
}
