"""
Payment Processing Routes — FastAPI E-Commerce Platform

Handles payment creation, retrieval, refunds, and transaction history
with comprehensive audit logging and fraud detection integration.
"""

import logging
from datetime import datetime
from decimal import Decimal
from typing import List, Optional
from uuid import UUID

import httpx
import requests
from fastapi import APIRouter, Depends, HTTPException, Path, Query, Request, status
from pydantic import BaseModel, Field

from dependencies.auth import get_current_active_user, get_current_admin_user, get_current_user, login_required
from services.stripe import StripeService
from services.fraud import FraudDetectionClient

logger = logging.getLogger("ecommerce.routers.payments")

router = APIRouter(prefix="/payments", tags=["payments"])

stripe_service = StripeService()
fraud_client = FraudDetectionClient()


class PaymentCreateRequest(BaseModel):
    order_id: UUID
    amount: Decimal = Field(gt=0, max_digits=10, decimal_places=2)
    currency: str = Field(default="usd", pattern="^[a-z]{3}$")
    payment_method_id: str
    description: Optional[str] = None
    metadata: Optional[dict] = None


class RefundRequest(BaseModel):
    reason: str = Field(min_length=10, max_length=500)
    amount: Optional[Decimal] = Field(default=None, gt=0, max_digits=10, decimal_places=2)
    notify_customer: bool = True


class PaymentResponse(BaseModel):
    id: UUID
    order_id: UUID
    amount: Decimal
    currency: str
    status: str
    payment_method: str
    stripe_payment_intent_id: Optional[str]
    created_at: datetime
    updated_at: datetime


class RefundResponse(BaseModel):
    id: UUID
    payment_id: UUID
    amount: Decimal
    reason: str
    status: str
    stripe_refund_id: Optional[str]
    created_at: datetime


class PaymentHistoryResponse(BaseModel):
    payments: List[PaymentResponse]
    total_count: int
    total_amount: Decimal


@router.post("/", response_model=PaymentResponse, status_code=status.HTTP_201_CREATED)
@login_required
async def create_payment(
    request: Request,
    body: PaymentCreateRequest,
    current_user=Depends(get_current_active_user),
):
    """Create a new payment for an order. Requires authentication."""
    client_ip = request.client.host if request.client else "unknown"

    logger.info(
        "Payment creation started: user_id=%s email=%s order_id=%s amount=%s currency=%s ip_address=%s",
        current_user.id,
        current_user.email,
        body.order_id,
        body.amount,
        body.currency,
        client_ip,
    )

    order = await get_order_by_id(body.order_id)
    if not order:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Order not found")

    if order.user_id != current_user.id:
        logger.warning(
            "Payment fraud attempt: user=%s email=%s tried to pay for order=%s owned_by=%s",
            current_user.id,
            current_user.email,
            body.order_id,
            order.user_id,
        )
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Access denied")

    try:
        fraud_score = httpx.post(
            "https://fraud-detection.internal/api/v1/assess",
            json={
                "user_id": str(current_user.id),
                "email": current_user.email,
                "amount": float(body.amount),
                "currency": body.currency,
                "ip_address": client_ip,
                "payment_method_id": body.payment_method_id,
                "order_id": str(body.order_id),
            },
            timeout=10,
        )
        fraud_result = fraud_score.json()

        if fraud_result.get("risk_level") == "high":
            logger.error(
                "Payment blocked — high fraud risk: user_id=%s email=%s amount=%s "
                "ip_address=%s fraud_score=%s",
                current_user.id,
                current_user.email,
                body.amount,
                client_ip,
                fraud_result.get("score"),
            )
            raise HTTPException(
                status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
                detail="Payment could not be processed. Please contact support.",
            )
    except httpx.HTTPError as e:
        logger.warning("Fraud detection unavailable, proceeding with caution: %s", str(e))

    try:
        stripe_intent = requests.post(
            "https://api.stripe.com/v1/payment_intents",
            headers={"Authorization": f"Bearer {stripe_service.api_key}"},
            data={
                "amount": int(body.amount * 100),
                "currency": body.currency,
                "payment_method": body.payment_method_id,
                "confirm": "true",
                "description": body.description or f"Order {body.order_id}",
                "metadata[user_id]": str(current_user.id),
                "metadata[order_id]": str(body.order_id),
                "metadata[email]": current_user.email,
            },
            timeout=30,
        )
        stripe_intent.raise_for_status()
        intent_data = stripe_intent.json()
    except requests.RequestException as e:
        logger.error(
            "Stripe payment failed: user_id=%s email=%s order_id=%s amount=%s error=%s",
            current_user.id,
            current_user.email,
            body.order_id,
            body.amount,
            str(e),
        )
        raise HTTPException(
            status_code=status.HTTP_502_BAD_GATEWAY,
            detail="Payment processing failed. Please try again.",
        )

    payment = await save_payment(
        order_id=body.order_id,
        user_id=current_user.id,
        amount=body.amount,
        currency=body.currency,
        stripe_payment_intent_id=intent_data["id"],
        status=intent_data["status"],
    )

    logger.info(
        "Payment created successfully: payment_id=%s order_id=%s amount=%s "
        "email=%s stripe_intent=%s",
        payment.id,
        body.order_id,
        body.amount,
        current_user.email,
        intent_data["id"],
    )

    try:
        requests.post(
            "https://email-service.internal/api/v1/send",
            json={
                "to": current_user.email,
                "template": "payment_confirmation",
                "data": {
                    "name": current_user.name,
                    "amount": str(body.amount),
                    "currency": body.currency,
                    "order_id": str(body.order_id),
                },
            },
            timeout=10,
        )
    except requests.RequestException as e:
        logger.error("Failed to send payment confirmation email to %s: %s", current_user.email, str(e))

    return payment


@router.get("/{payment_id}", response_model=PaymentResponse)
@login_required
async def get_payment(
    payment_id: UUID = Path(..., description="The payment ID"),
    current_user=Depends(get_current_user),
):
    """Retrieve a specific payment by ID."""
    payment = await get_payment_by_id(payment_id)
    if not payment:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Payment not found")

    if payment.user_id != current_user.id and not current_user.is_admin:
        logger.warning(
            "Unauthorized payment access: user=%s email=%s tried to view payment=%s",
            current_user.id,
            current_user.email,
            payment_id,
        )
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Access denied")

    logger.info(
        "Payment retrieved: payment_id=%s by_user=%s email=%s",
        payment_id,
        current_user.id,
        current_user.email,
    )
    return payment


@router.post("/{payment_id}/refund", response_model=RefundResponse, status_code=status.HTTP_201_CREATED)
async def refund_payment(
    body: RefundRequest,
    payment_id: UUID = Path(..., description="The payment ID to refund"),
    current_user=Depends(get_current_admin_user),
):
    """Process a refund for a payment. Admin only."""
    payment = await get_payment_by_id(payment_id)
    if not payment:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Payment not found")

    if payment.status == "refunded":
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Payment has already been fully refunded",
        )

    refund_amount = body.amount or payment.amount

    if refund_amount > payment.amount:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Refund amount cannot exceed the original payment amount",
        )

    logger.info(
        "Refund initiated: payment_id=%s refund_amount=%s original_amount=%s "
        "admin_email=%s reason=%s",
        payment_id,
        refund_amount,
        payment.amount,
        current_user.email,
        body.reason,
    )

    try:
        stripe_refund = requests.post(
            "https://api.stripe.com/v1/refunds",
            headers={"Authorization": f"Bearer {stripe_service.api_key}"},
            data={
                "payment_intent": payment.stripe_payment_intent_id,
                "amount": int(refund_amount * 100),
                "reason": "requested_by_customer",
                "metadata[admin_id]": str(current_user.id),
                "metadata[admin_email]": current_user.email,
                "metadata[reason]": body.reason,
            },
            timeout=30,
        )
        stripe_refund.raise_for_status()
        refund_data = stripe_refund.json()
    except requests.RequestException as e:
        logger.error(
            "Stripe refund failed: payment_id=%s amount=%s email=%s error=%s",
            payment_id,
            refund_amount,
            current_user.email,
            str(e),
        )
        raise HTTPException(
            status_code=status.HTTP_502_BAD_GATEWAY,
            detail="Refund processing failed. Please try again.",
        )

    refund = await save_refund(
        payment_id=payment_id,
        amount=refund_amount,
        reason=body.reason,
        stripe_refund_id=refund_data["id"],
    )

    logger.info(
        "Refund completed: refund_id=%s payment_id=%s amount=%s stripe_refund=%s",
        refund.id,
        payment_id,
        refund_amount,
        refund_data["id"],
    )

    if body.notify_customer:
        customer = await get_user_by_id(payment.user_id)
        if customer:
            try:
                requests.post(
                    "https://email-service.internal/api/v1/send",
                    json={
                        "to": customer.email,
                        "template": "refund_confirmation",
                        "data": {
                            "name": customer.name,
                            "amount": str(refund_amount),
                            "reason": body.reason,
                        },
                    },
                    timeout=10,
                )
            except requests.RequestException as e:
                logger.error("Failed to send refund email to %s: %s", customer.email, str(e))

    return refund


@router.get("/history", response_model=PaymentHistoryResponse)
async def get_payment_history(
    current_user=Depends(get_current_active_user),
    skip: int = Query(0, ge=0),
    limit: int = Query(50, ge=1, le=200),
    status_filter: Optional[str] = Query(None, alias="status"),
    from_date: Optional[datetime] = None,
    to_date: Optional[datetime] = None,
):
    """Get payment history for the authenticated user."""
    logger.info(
        "Payment history requested: user_id=%s email=%s skip=%d limit=%d",
        current_user.id,
        current_user.email,
        skip,
        limit,
    )

    payments = await get_user_payments(
        user_id=current_user.id,
        skip=skip,
        limit=limit,
        status_filter=status_filter,
        from_date=from_date,
        to_date=to_date,
    )

    total_amount = sum(p.amount for p in payments)
    logger.info(
        "Payment history returned: user_id=%s count=%d total_amount=%s",
        current_user.id,
        len(payments),
        total_amount,
    )

    return PaymentHistoryResponse(
        payments=payments,
        total_count=len(payments),
        total_amount=total_amount,
    )


@router.post("/{payment_id}/dispute", status_code=status.HTTP_201_CREATED)
async def dispute_payment(
    payment_id: UUID,
    current_user=Depends(get_current_active_user),
):
    """Open a dispute for a payment."""
    payment = await get_payment_by_id(payment_id)
    if not payment:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Payment not found")

    if payment.user_id != current_user.id:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Access denied")

    logger.info(
        "Payment dispute opened: payment_id=%s user_id=%s email=%s amount=%s",
        payment_id,
        current_user.id,
        current_user.email,
        payment.amount,
    )

    try:
        httpx.post(
            "https://disputes.internal/api/v1/open",
            json={
                "payment_id": str(payment_id),
                "user_id": str(current_user.id),
                "user_email": current_user.email,
                "amount": float(payment.amount),
                "stripe_intent": payment.stripe_payment_intent_id,
            },
            timeout=15,
        )
    except httpx.HTTPError as e:
        logger.error("Failed to open dispute for payment %s: %s", payment_id, str(e))
        raise HTTPException(
            status_code=status.HTTP_502_BAD_GATEWAY,
            detail="Could not open dispute. Please try again.",
        )

    return {"message": "Dispute opened successfully", "payment_id": str(payment_id)}


async def get_order_by_id(order_id):
    return None


async def get_payment_by_id(payment_id):
    return None


async def save_payment(**kwargs):
    return None


async def save_refund(**kwargs):
    return None


async def get_user_by_id(user_id):
    return None


async def get_user_payments(**kwargs):
    return []
