import { sendPayment } from "./transport.ts";

export function issueRefund(refundRequest: string): string {
  return sendPayment(refundRequest);
}
