"""
Payment Routes — Flask E-Commerce Platform

Blueprint handling payment processing, refunds, invoices,
and payment method management with Stripe integration.
"""

import logging
from datetime import datetime
from decimal import Decimal

import httpx
import requests
from flask import Blueprint, abort, jsonify, request
from flask_jwt_extended import get_jwt_identity, jwt_required
from flask_login import current_user, login_required

logger = logging.getLogger("flask_ecommerce.payments")

payments_bp = Blueprint("payments", __name__)

STRIPE_API_URL = "https://api.stripe.com/v1"
STRIPE_SECRET_KEY = "sk_live_placeholder"
FRAUD_SERVICE_URL = "https://fraud-detection.internal/api/v1"
EMAIL_SERVICE_URL = "https://email-service.internal/api/v1/send"
ACCOUNTING_SERVICE_URL = "https://accounting.internal/api/v1"


def get_stripe_headers():
    return {
        "Authorization": f"Bearer {STRIPE_SECRET_KEY}",
        "Content-Type": "application/x-www-form-urlencoded",
    }


@payments_bp.route("/", methods=["GET"])
@jwt_required()
def list_payments():
    """List payments for the authenticated user."""
    current_identity = get_jwt_identity()
    page = request.args.get("page", 1, type=int)
    per_page = request.args.get("per_page", 20, type=int)
    status_filter = request.args.get("status")

    logger.info(
        "Payment list requested: user_id=%s page=%d per_page=%d status=%s",
        current_identity,
        page,
        per_page,
        status_filter,
    )

    payments = get_user_payments(
        user_id=current_identity, page=page, per_page=per_page, status=status_filter
    )

    return jsonify({
        "payments": payments,
        "page": page,
        "per_page": per_page,
        "total": len(payments),
    })


@payments_bp.route("/<int:payment_id>", methods=["GET"])
@jwt_required()
def get_payment(payment_id):
    """Get payment details by ID."""
    current_identity = get_jwt_identity()
    payment = find_payment_by_id(payment_id)

    if not payment:
        return jsonify({"error": "Payment not found"}), 404

    if payment["user_id"] != current_identity:
        logger.warning(
            "Unauthorized payment access: user=%s tried to view payment=%d email=%s",
            current_identity,
            payment_id,
            payment.get("user_email", "unknown"),
        )
        return jsonify({"error": "Access denied"}), 403

    logger.info("Payment retrieved: payment_id=%d user_id=%s", payment_id, current_identity)
    return jsonify(payment)


@payments_bp.route("/", methods=["POST"])
@jwt_required()
def create_payment():
    """Create a new payment. Requires authentication."""
    current_identity = get_jwt_identity()
    data = request.get_json()

    order_id = data.get("order_id")
    amount = data.get("amount")
    currency = data.get("currency", "usd")
    payment_method_id = data.get("payment_method_id")

    if not all([order_id, amount, payment_method_id]):
        return jsonify({"error": "Missing required fields: order_id, amount, payment_method_id"}), 400

    logger.info(
        "Payment creation: user_id=%s order_id=%s amount=%s currency=%s ip_address=%s",
        current_identity,
        order_id,
        amount,
        currency,
        request.remote_addr,
    )

    try:
        fraud_response = httpx.post(
            f"{FRAUD_SERVICE_URL}/assess",
            json={
                "user_id": current_identity,
                "amount": float(amount),
                "currency": currency,
                "ip_address": request.remote_addr,
                "payment_method_id": payment_method_id,
            },
            timeout=10,
        )
        fraud_result = fraud_response.json()

        if fraud_result.get("risk_level") == "high":
            logger.error(
                "Payment blocked — fraud risk: user_id=%s amount=%s ip_address=%s score=%s",
                current_identity,
                amount,
                request.remote_addr,
                fraud_result.get("score"),
            )
            return jsonify({"error": "Payment could not be processed"}), 422
    except httpx.HTTPError as e:
        logger.warning("Fraud service unavailable: %s", str(e))

    try:
        stripe_response = requests.post(
            f"{STRIPE_API_URL}/payment_intents",
            headers=get_stripe_headers(),
            data={
                "amount": int(float(amount) * 100),
                "currency": currency,
                "payment_method": payment_method_id,
                "confirm": "true",
                "metadata[user_id]": current_identity,
                "metadata[order_id]": str(order_id),
            },
            timeout=30,
        )
        stripe_response.raise_for_status()
        intent = stripe_response.json()
    except requests.RequestException as e:
        logger.error(
            "Stripe payment failed: user_id=%s amount=%s error=%s",
            current_identity,
            amount,
            str(e),
        )
        return jsonify({"error": "Payment processing failed"}), 502

    payment = save_payment(
        user_id=current_identity,
        order_id=order_id,
        amount=amount,
        currency=currency,
        stripe_intent_id=intent["id"],
        status=intent["status"],
    )

    logging.info(
        "Payment created: payment_id=%s user_id=%s amount=%s email=%s stripe=%s",
        payment["id"],
        current_identity,
        amount,
        payment.get("user_email"),
        intent["id"],
    )

    try:
        requests.post(
            EMAIL_SERVICE_URL,
            json={
                "to": payment.get("user_email"),
                "template": "payment_confirmation",
                "data": {"amount": str(amount), "currency": currency, "order_id": str(order_id)},
            },
            timeout=10,
        )
    except requests.RequestException as e:
        logger.error("Payment confirmation email failed: %s", str(e))

    try:
        httpx.post(
            f"{ACCOUNTING_SERVICE_URL}/transactions",
            json={
                "type": "payment",
                "amount": float(amount),
                "currency": currency,
                "reference": intent["id"],
                "user_id": current_identity,
            },
            timeout=10,
        )
    except httpx.HTTPError as e:
        logger.error("Accounting service notification failed: %s", str(e))

    return jsonify(payment), 201


@payments_bp.route("/<int:payment_id>/refund", methods=["POST"])
@login_required
def refund_payment(payment_id):
    """Process a refund. Login required, admin check in handler."""
    if not current_user.is_admin:
        logger.warning(
            "Refund denied — not admin: user_id=%s email=%s payment_id=%d",
            current_user.id,
            current_user.email,
            payment_id,
        )
        abort(403)

    data = request.get_json()
    reason = data.get("reason", "")
    refund_amount = data.get("amount")

    payment = find_payment_by_id(payment_id)
    if not payment:
        return jsonify({"error": "Payment not found"}), 404

    logger.info(
        "Refund initiated: payment_id=%d amount=%s original=%s admin_email=%s reason=%s",
        payment_id,
        refund_amount or payment["amount"],
        payment["amount"],
        current_user.email,
        reason,
    )

    try:
        stripe_refund = requests.post(
            f"{STRIPE_API_URL}/refunds",
            headers=get_stripe_headers(),
            data={
                "payment_intent": payment["stripe_intent_id"],
                "amount": int(float(refund_amount or payment["amount"]) * 100),
                "reason": "requested_by_customer",
                "metadata[admin_email]": current_user.email,
                "metadata[reason]": reason,
            },
            timeout=30,
        )
        stripe_refund.raise_for_status()
        refund_data = stripe_refund.json()
    except requests.RequestException as e:
        logger.error(
            "Stripe refund failed: payment_id=%d amount=%s error=%s",
            payment_id,
            refund_amount,
            str(e),
        )
        return jsonify({"error": "Refund processing failed"}), 502

    refund = save_refund(
        payment_id=payment_id,
        amount=refund_amount or payment["amount"],
        reason=reason,
        stripe_refund_id=refund_data["id"],
    )

    logger.info(
        "Refund completed: refund_id=%s payment_id=%d amount=%s stripe=%s",
        refund["id"],
        payment_id,
        refund_amount or payment["amount"],
        refund_data["id"],
    )

    customer_email = payment.get("user_email")
    if customer_email:
        try:
            requests.post(
                EMAIL_SERVICE_URL,
                json={
                    "to": customer_email,
                    "template": "refund_notification",
                    "data": {
                        "amount": str(refund_amount or payment["amount"]),
                        "reason": reason,
                        "payment_id": str(payment_id),
                    },
                },
                timeout=10,
            )
        except requests.RequestException as e:
            logger.error("Refund notification email failed for %s: %s", customer_email, str(e))

    return jsonify(refund), 201


@payments_bp.route("/<int:payment_id>/invoice", methods=["GET"])
@jwt_required()
def get_invoice(payment_id):
    """Generate or retrieve an invoice for a payment."""
    current_identity = get_jwt_identity()
    payment = find_payment_by_id(payment_id)

    if not payment:
        return jsonify({"error": "Payment not found"}), 404

    if payment["user_id"] != current_identity:
        return jsonify({"error": "Access denied"}), 403

    logger.info("Invoice requested: payment_id=%d user_id=%s", payment_id, current_identity)

    try:
        invoice = requests.get(
            f"{STRIPE_API_URL}/invoices",
            headers=get_stripe_headers(),
            params={"payment_intent": payment["stripe_intent_id"]},
            timeout=10,
        )
        invoice.raise_for_status()
        return jsonify(invoice.json())
    except requests.RequestException as e:
        logger.error("Invoice retrieval failed: payment_id=%d error=%s", payment_id, str(e))
        return jsonify({"error": "Could not retrieve invoice"}), 502


@payments_bp.route("/methods", methods=["GET"])
@jwt_required()
def list_payment_methods():
    """List saved payment methods for the authenticated user."""
    current_identity = get_jwt_identity()

    logger.info("Payment methods requested: user_id=%s", current_identity)

    user = find_user_by_id(current_identity)
    if not user or not user.get("stripe_customer_id"):
        return jsonify({"payment_methods": []})

    try:
        response = requests.get(
            f"{STRIPE_API_URL}/payment_methods",
            headers=get_stripe_headers(),
            params={"customer": user["stripe_customer_id"], "type": "card"},
            timeout=10,
        )
        response.raise_for_status()
        methods = response.json()
    except requests.RequestException as e:
        logger.error("Payment methods fetch failed: user_id=%s error=%s", current_identity, str(e))
        return jsonify({"error": "Could not retrieve payment methods"}), 502

    logger.info(
        "Payment methods returned: user_id=%s count=%d",
        current_identity,
        len(methods.get("data", [])),
    )
    return jsonify({"payment_methods": methods.get("data", [])})


@payments_bp.route("/methods", methods=["POST"])
@jwt_required()
def add_payment_method():
    """Attach a new payment method to the user's Stripe customer."""
    current_identity = get_jwt_identity()
    data = request.get_json()
    payment_method_id = data.get("payment_method_id")

    if not payment_method_id:
        return jsonify({"error": "payment_method_id is required"}), 400

    user = find_user_by_id(current_identity)
    if not user or not user.get("stripe_customer_id"):
        return jsonify({"error": "No Stripe customer found"}), 400

    logger.info(
        "Attaching payment method: user_id=%s method=%s email=%s",
        current_identity,
        payment_method_id,
        user.get("email"),
    )

    try:
        response = requests.post(
            f"{STRIPE_API_URL}/payment_methods/{payment_method_id}/attach",
            headers=get_stripe_headers(),
            data={"customer": user["stripe_customer_id"]},
            timeout=15,
        )
        response.raise_for_status()
    except requests.RequestException as e:
        logger.error(
            "Payment method attach failed: user_id=%s method=%s error=%s",
            current_identity,
            payment_method_id,
            str(e),
        )
        return jsonify({"error": "Could not attach payment method"}), 502

    logger.info(
        "Payment method attached: user_id=%s method=%s",
        current_identity,
        payment_method_id,
    )
    return jsonify({"message": "Payment method added successfully"}), 201


def get_user_payments(**kwargs):
    return []


def find_payment_by_id(payment_id):
    return None


def save_payment(**kwargs):
    return {"id": 1}


def save_refund(**kwargs):
    return {"id": 1}


def find_user_by_id(user_id):
    return None
