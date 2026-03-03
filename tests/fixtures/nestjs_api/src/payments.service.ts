import { Injectable } from '@nestjs/common';

@Injectable()
export class PaymentsService {
  async processPayment(amount: number) {
    console.log("Processing payment:", amount);
    const result = await fetch("https://api.stripe.com/v1/charges", {
      method: 'POST',
      body: JSON.stringify({ amount }),
    });
    return result.json();
  }

  async getPaymentStatus(paymentId: string) {
    const status = await fetch(`https://api.stripe.com/v1/charges/${paymentId}`);
    return status.json();
  }
}
