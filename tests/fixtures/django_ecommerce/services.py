"""
Django E-Commerce Services

Service layer encapsulating external API integrations for the
e-commerce platform: payments, shipping, notifications, analytics,
inventory, and identity verification.
"""

import logging
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal
from typing import Any, Dict, List, Optional

import httpx
import requests

logger = logging.getLogger("ecommerce.services")
log = logging.getLogger("ecommerce.services.external")

STRIPE_API_URL = "https://api.stripe.com/v1"
PAYPAL_API_URL = "https://api.paypal.com/v2"
SHIPPING_API_URL = "https://shipping-provider.com/api/v1"
EMAIL_SERVICE_URL = "https://email-service.internal/api/v1"
SMS_SERVICE_URL = "https://sms-gateway.internal/api/v1"
ANALYTICS_URL = "https://analytics.internal/api/v1"
INVENTORY_URL = "https://inventory.internal/api/v1"
IDENTITY_URL = "https://identity-verification.com/api/v2"
FRAUD_URL = "https://fraud-detection.internal/api/v1"
TAX_SERVICE_URL = "https://tax-calculator.com/api/v1"


@dataclass
class PaymentResult:
    provider: str
    transaction_id: str
    amount: Decimal
    currency: str
    status: str
    metadata: Dict[str, Any]


@dataclass
class ShippingQuote:
    carrier: str
    service: str
    estimated_days: int
    cost: Decimal
    tracking_number: Optional[str] = None


class PaymentService:
    """Handles payment processing across multiple providers."""

    def __init__(self, stripe_key: str, paypal_client_id: str = "", paypal_secret: str = ""):
        self.stripe_key = stripe_key
        self.paypal_client_id = paypal_client_id
        self.paypal_secret = paypal_secret
        self._stripe_headers = {
            "Authorization": f"Bearer {stripe_key}",
            "Content-Type": "application/x-www-form-urlencoded",
        }

    def charge_stripe(
        self,
        amount: Decimal,
        currency: str,
        payment_method_id: str,
        customer_email: str,
        order_id: str,
        metadata: Optional[Dict] = None,
    ) -> PaymentResult:
        """Process a payment through Stripe."""
        logger.info(
            "Stripe charge: amount=%s currency=%s email=%s order_id=%s",
            amount,
            currency,
            customer_email,
            order_id,
        )

        data = {
            "amount": str(int(amount * 100)),
            "currency": currency,
            "payment_method": payment_method_id,
            "confirm": "true",
            "metadata[email]": customer_email,
            "metadata[order_id]": order_id,
        }
        if metadata:
            for k, v in metadata.items():
                data[f"metadata[{k}]"] = str(v)

        try:
            response = requests.post(
                f"{STRIPE_API_URL}/payment_intents",
                headers=self._stripe_headers,
                data=data,
                timeout=30,
            )
            response.raise_for_status()
            intent = response.json()
        except requests.RequestException as e:
            logger.error(
                "Stripe charge failed: amount=%s email=%s credit_card=%s error=%s",
                amount,
                customer_email,
                payment_method_id,
                str(e),
            )
            raise PaymentError(f"Stripe payment failed: {e}")

        logger.info(
            "Stripe charge successful: intent=%s amount=%s email=%s status=%s",
            intent["id"],
            amount,
            customer_email,
            intent["status"],
        )

        return PaymentResult(
            provider="stripe",
            transaction_id=intent["id"],
            amount=amount,
            currency=currency,
            status=intent["status"],
            metadata=intent.get("metadata", {}),
        )

    def charge_paypal(
        self,
        amount: Decimal,
        currency: str,
        customer_email: str,
        order_id: str,
    ) -> PaymentResult:
        """Process a payment through PayPal."""
        logger.info(
            "PayPal charge: amount=%s currency=%s email=%s order_id=%s",
            amount,
            currency,
            customer_email,
            order_id,
        )

        try:
            auth_response = requests.post(
                f"{PAYPAL_API_URL}/oauth2/token",
                data={"grant_type": "client_credentials"},
                auth=(self.paypal_client_id, self.paypal_secret),
                timeout=10,
            )
            auth_response.raise_for_status()
            access_token = auth_response.json()["access_token"]
        except requests.RequestException as e:
            logger.error("PayPal auth failed: %s", str(e))
            raise PaymentError(f"PayPal authentication failed: {e}")

        try:
            order_response = requests.post(
                f"{PAYPAL_API_URL}/checkout/orders",
                headers={
                    "Authorization": f"Bearer {access_token}",
                    "Content-Type": "application/json",
                },
                json={
                    "intent": "CAPTURE",
                    "purchase_units": [{
                        "amount": {"currency_code": currency.upper(), "value": str(amount)},
                        "custom_id": order_id,
                    }],
                    "payer": {"email_address": customer_email},
                },
                timeout=30,
            )
            order_response.raise_for_status()
            order_data = order_response.json()
        except requests.RequestException as e:
            logger.error(
                "PayPal order creation failed: amount=%s email=%s error=%s",
                amount,
                customer_email,
                str(e),
            )
            raise PaymentError(f"PayPal payment failed: {e}")

        log.info(
            "PayPal charge successful: order=%s amount=%s email=%s",
            order_data["id"],
            amount,
            customer_email,
        )

        return PaymentResult(
            provider="paypal",
            transaction_id=order_data["id"],
            amount=amount,
            currency=currency,
            status=order_data["status"],
            metadata={"paypal_order_id": order_data["id"]},
        )

    def refund_stripe(
        self, payment_intent_id: str, amount: Optional[Decimal] = None, admin_email: str = ""
    ) -> Dict:
        """Process a Stripe refund."""
        logger.info(
            "Stripe refund: intent=%s amount=%s admin_email=%s",
            payment_intent_id,
            amount,
            admin_email,
        )

        data = {"payment_intent": payment_intent_id}
        if amount:
            data["amount"] = str(int(amount * 100))
        if admin_email:
            data["metadata[admin_email]"] = admin_email

        try:
            response = requests.post(
                f"{STRIPE_API_URL}/refunds",
                headers=self._stripe_headers,
                data=data,
                timeout=30,
            )
            response.raise_for_status()
            refund = response.json()
        except requests.RequestException as e:
            logger.error(
                "Stripe refund failed: intent=%s amount=%s error=%s",
                payment_intent_id,
                amount,
                str(e),
            )
            raise PaymentError(f"Stripe refund failed: {e}")

        logger.info("Stripe refund completed: refund_id=%s amount=%s", refund["id"], refund["amount"])
        return refund


class ShippingService:
    """Manages shipping quotes, label creation, and tracking."""

    def get_quotes(
        self, origin_zip: str, destination_zip: str, weight_kg: float, dimensions: Dict
    ) -> List[ShippingQuote]:
        """Get shipping quotes from the shipping provider."""
        logger.info(
            "Shipping quote: origin=%s destination=%s weight=%skg",
            origin_zip,
            destination_zip,
            weight_kg,
        )

        try:
            response = requests.post(
                f"{SHIPPING_API_URL}/quotes",
                json={
                    "origin": {"zip": origin_zip},
                    "destination": {"zip": destination_zip},
                    "package": {"weight_kg": weight_kg, **dimensions},
                },
                timeout=15,
            )
            response.raise_for_status()
            quotes_data = response.json()
        except requests.RequestException as e:
            logger.error("Shipping quote failed: %s", str(e))
            return []

        return [
            ShippingQuote(
                carrier=q["carrier"],
                service=q["service"],
                estimated_days=q["estimated_days"],
                cost=Decimal(str(q["cost"])),
            )
            for q in quotes_data.get("quotes", [])
        ]

    def create_label(
        self, order_id: str, carrier: str, service: str, recipient_name: str, recipient_email: str, address: Dict
    ) -> Dict:
        """Create a shipping label."""
        logger.info(
            "Creating shipping label: order=%s carrier=%s recipient=%s email=%s",
            order_id,
            carrier,
            recipient_name,
            recipient_email,
        )

        try:
            response = requests.post(
                f"{SHIPPING_API_URL}/labels",
                json={
                    "order_id": order_id,
                    "carrier": carrier,
                    "service": service,
                    "recipient": {
                        "name": recipient_name,
                        "email": recipient_email,
                        **address,
                    },
                },
                timeout=20,
            )
            response.raise_for_status()
            label = response.json()
        except requests.RequestException as e:
            logger.error("Shipping label creation failed: order=%s error=%s", order_id, str(e))
            raise ShippingError(f"Label creation failed: {e}")

        logger.info(
            "Shipping label created: order=%s tracking=%s email=%s",
            order_id,
            label.get("tracking_number"),
            recipient_email,
        )
        return label

    def get_tracking(self, tracking_number: str) -> Dict:
        """Get tracking information for a shipment."""
        try:
            response = httpx.get(
                f"{SHIPPING_API_URL}/tracking/{tracking_number}",
                timeout=10,
            )
            return response.json()
        except httpx.HTTPError as e:
            logger.error("Tracking lookup failed: tracking=%s error=%s", tracking_number, str(e))
            return {"status": "unknown", "error": str(e)}


class NotificationService:
    """Handles email and SMS notifications."""

    def send_email(self, to_email: str, template: str, data: Dict) -> bool:
        """Send a transactional email."""
        logger.info("Sending email: to=%s template=%s", to_email, template)

        try:
            response = requests.post(
                f"{EMAIL_SERVICE_URL}/send",
                json={"to": to_email, "template": template, "data": data},
                timeout=10,
            )
            response.raise_for_status()
            logger.info("Email sent successfully: to=%s template=%s", to_email, template)
            return True
        except requests.RequestException as e:
            logger.error("Email send failed: to=%s template=%s error=%s", to_email, template, str(e))
            return False

    def send_sms(self, phone_number: str, message: str) -> bool:
        """Send an SMS notification."""
        log.info("Sending SMS: phone=%s message_length=%d", phone_number, len(message))

        try:
            response = requests.post(
                f"{SMS_SERVICE_URL}/send",
                json={"to": phone_number, "message": message},
                timeout=10,
            )
            response.raise_for_status()
            log.info("SMS sent: phone=%s", phone_number)
            return True
        except requests.RequestException as e:
            log.error("SMS failed: phone=%s error=%s", phone_number, str(e))
            return False

    def send_order_confirmation(self, email: str, name: str, order_id: str, total: str) -> bool:
        """Send order confirmation via email."""
        return self.send_email(email, "order_confirmation", {
            "name": name,
            "order_id": order_id,
            "total": total,
        })


class AnalyticsService:
    """Tracks business events for analytics and reporting."""

    def track_event(self, event_name: str, properties: Dict) -> None:
        """Send an analytics event."""
        try:
            httpx.post(
                f"{ANALYTICS_URL}/track",
                json={"event": event_name, "properties": properties, "timestamp": datetime.utcnow().isoformat()},
                timeout=5,
            )
        except httpx.HTTPError as e:
            logger.warning("Analytics event failed: event=%s error=%s", event_name, str(e))

    def track_purchase(self, user_email: str, order_id: str, amount: float, currency: str) -> None:
        """Track a purchase event."""
        logger.info(
            "Purchase tracked: email=%s order=%s amount=%s currency=%s",
            user_email,
            order_id,
            amount,
            currency,
        )
        self.track_event("purchase", {
            "email": user_email,
            "order_id": order_id,
            "amount": amount,
            "currency": currency,
        })

    def track_page_view(self, path: str, user_email: str, ip_address: str) -> None:
        """Track a page view."""
        logger.info("Page view: path=%s email=%s ip_address=%s", path, user_email, ip_address)
        try:
            httpx.post(
                f"{ANALYTICS_URL}/pageviews",
                json={"path": path, "user": user_email, "ip": ip_address},
                timeout=5,
            )
        except httpx.HTTPError:
            pass


class FraudDetectionService:
    """Assesses fraud risk for transactions."""

    def assess_risk(
        self, user_id: str, email: str, amount: float, ip_address: str, payment_method: str
    ) -> Dict:
        """Assess fraud risk for a transaction."""
        logger.info(
            "Fraud assessment: user_id=%s email=%s amount=%s ip_address=%s",
            user_id,
            email,
            amount,
            ip_address,
        )

        try:
            response = httpx.post(
                f"{FRAUD_URL}/assess",
                json={
                    "user_id": user_id,
                    "email": email,
                    "amount": amount,
                    "ip_address": ip_address,
                    "payment_method": payment_method,
                },
                timeout=10,
            )
            result = response.json()
        except httpx.HTTPError as e:
            logger.error(
                "Fraud assessment failed: user_id=%s email=%s ip_address=%s error=%s",
                user_id,
                email,
                ip_address,
                str(e),
            )
            return {"risk_level": "unknown", "score": 0}

        if result.get("risk_level") == "high":
            logger.error(
                "High fraud risk detected: user_id=%s email=%s amount=%s ip_address=%s score=%s",
                user_id,
                email,
                amount,
                ip_address,
                result.get("score"),
            )

        return result

    def check_transaction(self, user_email: str, amount: float, ip_address: str, card_last4: str) -> int:
        """Legacy fraud check interface."""
        logger.info(
            "Fraud check: email=%s amount=%s ip_address=%s credit_card=%s",
            user_email,
            amount,
            ip_address,
            card_last4,
        )

        try:
            response = requests.post(
                f"{FRAUD_URL}/check",
                json={
                    "email": user_email,
                    "amount": amount,
                    "ip_address": ip_address,
                    "card_last4": card_last4,
                },
                timeout=10,
            )
            score = response.json().get("risk_score", 0)
        except requests.RequestException as e:
            logger.error("Fraud check failed: email=%s error=%s", user_email, str(e))
            return 0

        if score > 80:
            logger.warn(
                "High fraud risk: email=%s score=%s ip_address=%s",
                user_email,
                score,
                ip_address,
            )

        return score


class IdentityVerificationService:
    """Handles KYC/identity verification for high-value transactions."""

    def verify_identity(self, user_id: str, name: str, email: str, ssn_last_four: str) -> Dict:
        """Submit identity verification."""
        logger.info(
            "Identity verification: user_id=%s name=%s email=%s ssn=%s",
            user_id,
            name,
            email,
            ssn_last_four,
        )

        try:
            response = requests.post(
                f"{IDENTITY_URL}/verify",
                json={
                    "user_id": user_id,
                    "full_name": name,
                    "email": email,
                    "ssn_last_four": ssn_last_four,
                },
                timeout=15,
            )
            response.raise_for_status()
            result = response.json()
        except requests.RequestException as e:
            logger.error(
                "Identity verification failed: user_id=%s email=%s error=%s",
                user_id,
                email,
                str(e),
            )
            return {"verified": False, "error": str(e)}

        logger.info(
            "Identity verification result: user_id=%s email=%s verified=%s",
            user_id,
            email,
            result.get("verified"),
        )
        return result


class TaxService:
    """Calculates taxes for orders."""

    def calculate_tax(self, subtotal: Decimal, shipping: Decimal, zip_code: str) -> Dict:
        """Calculate tax for an order."""
        try:
            response = httpx.post(
                f"{TAX_SERVICE_URL}/calculate",
                json={
                    "subtotal": float(subtotal),
                    "shipping": float(shipping),
                    "destination_zip": zip_code,
                },
                timeout=5,
            )
            result = response.json()
            logger.info("Tax calculated: subtotal=%s tax=%s zip=%s", subtotal, result.get("tax"), zip_code)
            return result
        except httpx.HTTPError as e:
            logger.error("Tax calculation failed: subtotal=%s zip=%s error=%s", subtotal, zip_code, str(e))
            return {"tax": 0, "rate": 0, "error": str(e)}


class PaymentError(Exception):
    pass


class ShippingError(Exception):
    pass
