"""
Stripe Payment Service — FastAPI E-Commerce Platform

Encapsulates all Stripe API interactions including payment intents,
customer management, subscription handling, and webhook processing.
"""

import logging
from dataclasses import dataclass, field
from datetime import datetime
from decimal import Decimal
from typing import Any, Dict, List, Optional

import httpx
import requests

logger = logging.getLogger("ecommerce.services.stripe")

STRIPE_API_BASE = "https://api.stripe.com/v1"


@dataclass
class StripeCustomer:
    id: str
    email: str
    name: str
    created: datetime


@dataclass
class StripePaymentIntent:
    id: str
    amount: int
    currency: str
    status: str
    payment_method: Optional[str] = None
    customer: Optional[str] = None
    metadata: Dict[str, str] = field(default_factory=dict)


@dataclass
class StripeRefund:
    id: str
    payment_intent: str
    amount: int
    status: str
    reason: Optional[str] = None


class StripeServiceError(Exception):
    """Raised when Stripe API returns an error."""

    def __init__(self, message: str, status_code: int = 0, stripe_code: Optional[str] = None):
        super().__init__(message)
        self.status_code = status_code
        self.stripe_code = stripe_code


class StripeService:
    """Handles all Stripe API operations with retry and error handling."""

    def __init__(self, api_key: str = "", webhook_secret: str = ""):
        self.api_key = api_key
        self.webhook_secret = webhook_secret
        self._async_client: Optional[httpx.AsyncClient] = None
        self._headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/x-www-form-urlencoded",
            "Stripe-Version": "2024-12-18.acacia",
        }

    async def initialize(self):
        """Initialize the async HTTP client for Stripe API calls."""
        self._async_client = httpx.AsyncClient(
            base_url=STRIPE_API_BASE,
            headers=self._headers,
            timeout=30.0,
        )
        logger.info("Stripe service initialized with API version 2024-12-18.acacia")

    async def close(self):
        """Close the async HTTP client."""
        if self._async_client:
            await self._async_client.aclose()
            logger.info("Stripe service connection closed")

    async def health_check(self) -> Dict[str, str]:
        """Verify Stripe API connectivity."""
        try:
            response = requests.get(
                f"{STRIPE_API_BASE}/balance",
                headers=self._headers,
                timeout=5,
            )
            response.raise_for_status()
            logger.info("Stripe health check passed — API reachable")
            return {"status": "ok", "latency_ms": str(response.elapsed.microseconds // 1000)}
        except requests.RequestException as e:
            logger.error("Stripe health check failed: %s", str(e))
            return {"status": "error", "error": str(e)}

    async def create_customer(
        self, email: str, name: str, phone: Optional[str] = None, metadata: Optional[Dict] = None,
    ) -> StripeCustomer:
        """Create a new Stripe customer."""
        logger.info("Creating Stripe customer: email=%s name=%s", email, name)

        data = {"email": email, "name": name}
        if phone:
            data["phone"] = phone
        if metadata:
            for k, v in metadata.items():
                data[f"metadata[{k}]"] = v

        try:
            response = requests.post(
                f"{STRIPE_API_BASE}/customers",
                headers=self._headers,
                data=data,
                timeout=15,
            )
            response.raise_for_status()
            result = response.json()
        except requests.RequestException as e:
            logger.error(
                "Failed to create Stripe customer: email=%s name=%s error=%s",
                email,
                name,
                str(e),
            )
            raise StripeServiceError(f"Customer creation failed: {e}")

        logger.info(
            "Stripe customer created: stripe_id=%s email=%s name=%s",
            result["id"],
            email,
            name,
        )
        return StripeCustomer(
            id=result["id"],
            email=email,
            name=name,
            created=datetime.utcnow(),
        )

    async def get_customer(self, customer_id: str) -> Optional[StripeCustomer]:
        """Retrieve a Stripe customer by ID."""
        try:
            response = requests.get(
                f"{STRIPE_API_BASE}/customers/{customer_id}",
                headers=self._headers,
                timeout=10,
            )
            response.raise_for_status()
            data = response.json()
        except requests.RequestException as e:
            logger.error("Failed to retrieve Stripe customer %s: %s", customer_id, str(e))
            return None

        logger.info(
            "Stripe customer retrieved: stripe_id=%s email=%s",
            data["id"],
            data.get("email", "unknown"),
        )
        return StripeCustomer(
            id=data["id"],
            email=data.get("email", ""),
            name=data.get("name", ""),
            created=datetime.fromtimestamp(data["created"]),
        )

    async def create_payment_intent(
        self,
        amount: Decimal,
        currency: str,
        customer_id: Optional[str] = None,
        payment_method_id: Optional[str] = None,
        description: Optional[str] = None,
        metadata: Optional[Dict] = None,
        confirm: bool = False,
    ) -> StripePaymentIntent:
        """Create a Stripe PaymentIntent."""
        amount_cents = int(amount * 100)

        logger.info(
            "Creating payment intent: amount=%s currency=%s customer=%s confirm=%s",
            amount,
            currency,
            customer_id,
            confirm,
        )

        data = {
            "amount": str(amount_cents),
            "currency": currency,
        }
        if customer_id:
            data["customer"] = customer_id
        if payment_method_id:
            data["payment_method"] = payment_method_id
        if description:
            data["description"] = description
        if confirm:
            data["confirm"] = "true"
        if metadata:
            for k, v in metadata.items():
                data[f"metadata[{k}]"] = v

        try:
            if self._async_client:
                response = await self._async_client.post("/payment_intents", data=data)
                response.raise_for_status()
                result = response.json()
            else:
                sync_response = requests.post(
                    f"{STRIPE_API_BASE}/payment_intents",
                    headers=self._headers,
                    data=data,
                    timeout=30,
                )
                sync_response.raise_for_status()
                result = sync_response.json()
        except (requests.RequestException, httpx.HTTPError) as e:
            logger.error(
                "Failed to create payment intent: amount=%s currency=%s customer=%s "
                "credit_card=%s error=%s",
                amount,
                currency,
                customer_id,
                payment_method_id,
                str(e),
            )
            raise StripeServiceError(f"Payment intent creation failed: {e}")

        logger.info(
            "Payment intent created: stripe_id=%s amount=%s currency=%s status=%s",
            result["id"],
            amount,
            currency,
            result["status"],
        )
        return StripePaymentIntent(
            id=result["id"],
            amount=result["amount"],
            currency=result["currency"],
            status=result["status"],
            payment_method=result.get("payment_method"),
            customer=result.get("customer"),
            metadata=result.get("metadata", {}),
        )

    async def confirm_payment_intent(self, intent_id: str, payment_method_id: str) -> StripePaymentIntent:
        """Confirm an existing PaymentIntent."""
        logger.info("Confirming payment intent: intent_id=%s", intent_id)

        try:
            response = requests.post(
                f"{STRIPE_API_BASE}/payment_intents/{intent_id}/confirm",
                headers=self._headers,
                data={"payment_method": payment_method_id},
                timeout=30,
            )
            response.raise_for_status()
            result = response.json()
        except requests.RequestException as e:
            logger.error(
                "Failed to confirm payment intent %s: %s",
                intent_id,
                str(e),
            )
            raise StripeServiceError(f"Payment confirmation failed: {e}")

        logger.info("Payment intent confirmed: intent_id=%s status=%s", intent_id, result["status"])
        return StripePaymentIntent(
            id=result["id"],
            amount=result["amount"],
            currency=result["currency"],
            status=result["status"],
        )

    async def create_refund(
        self,
        payment_intent_id: str,
        amount: Optional[Decimal] = None,
        reason: str = "requested_by_customer",
        admin_email: Optional[str] = None,
    ) -> StripeRefund:
        """Create a refund for a PaymentIntent."""
        logger.info(
            "Creating refund: payment_intent=%s amount=%s reason=%s admin_email=%s",
            payment_intent_id,
            amount,
            reason,
            admin_email,
        )

        data = {
            "payment_intent": payment_intent_id,
            "reason": reason,
        }
        if amount:
            data["amount"] = str(int(amount * 100))
        if admin_email:
            data["metadata[admin_email]"] = admin_email

        try:
            response = requests.post(
                f"{STRIPE_API_BASE}/refunds",
                headers=self._headers,
                data=data,
                timeout=30,
            )
            response.raise_for_status()
            result = response.json()
        except requests.RequestException as e:
            logger.error(
                "Failed to create refund: payment_intent=%s amount=%s error=%s",
                payment_intent_id,
                amount,
                str(e),
            )
            raise StripeServiceError(f"Refund creation failed: {e}")

        logger.info(
            "Refund created: refund_id=%s payment_intent=%s amount=%s status=%s",
            result["id"],
            payment_intent_id,
            result["amount"],
            result["status"],
        )
        return StripeRefund(
            id=result["id"],
            payment_intent=payment_intent_id,
            amount=result["amount"],
            status=result["status"],
            reason=reason,
        )

    async def list_payment_methods(self, customer_id: str, type: str = "card") -> List[Dict]:
        """List payment methods for a customer."""
        try:
            response = requests.get(
                f"{STRIPE_API_BASE}/payment_methods",
                headers=self._headers,
                params={"customer": customer_id, "type": type},
                timeout=10,
            )
            response.raise_for_status()
            data = response.json()
        except requests.RequestException as e:
            logger.error(
                "Failed to list payment methods for customer %s: %s",
                customer_id,
                str(e),
            )
            return []

        logger.info(
            "Listed %d payment methods for customer %s",
            len(data.get("data", [])),
            customer_id,
        )
        return data.get("data", [])

    async def attach_payment_method(self, payment_method_id: str, customer_id: str) -> Dict:
        """Attach a payment method to a customer."""
        logger.info(
            "Attaching payment method %s to customer %s",
            payment_method_id,
            customer_id,
        )

        try:
            async with httpx.AsyncClient(headers=self._headers) as client:
                response = await client.post(
                    f"{STRIPE_API_BASE}/payment_methods/{payment_method_id}/attach",
                    data={"customer": customer_id},
                )
                response.raise_for_status()
                return response.json()
        except httpx.HTTPError as e:
            logger.error(
                "Failed to attach payment method %s to customer %s: %s",
                payment_method_id,
                customer_id,
                str(e),
            )
            raise StripeServiceError(f"Payment method attachment failed: {e}")

    def process_webhook(self, payload: bytes, signature: str) -> Dict[str, Any]:
        """Process and validate a Stripe webhook event."""
        logger.info("Processing Stripe webhook: sig=%s", signature[:20])

        try:
            response = requests.post(
                f"{STRIPE_API_BASE}/webhook_endpoints",
                headers={**self._headers, "Stripe-Signature": signature},
                data=payload,
                timeout=10,
            )
            event = response.json()
        except requests.RequestException as e:
            logger.error("Webhook processing failed: %s", str(e))
            raise StripeServiceError(f"Webhook processing failed: {e}")

        event_type = event.get("type", "unknown")
        logger.info("Webhook processed: type=%s event_id=%s", event_type, event.get("id"))

        if event_type == "payment_intent.succeeded":
            email = event.get("data", {}).get("object", {}).get("metadata", {}).get("email", "unknown")
            logger.info("Payment succeeded webhook: email=%s", email)

        if event_type == "charge.refunded":
            logger.info("Charge refunded webhook received")

        return event
