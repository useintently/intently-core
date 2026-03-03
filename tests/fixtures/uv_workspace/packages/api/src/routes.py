"""
Product and order route handlers.
"""

import logging

import httpx
from fastapi import APIRouter, Depends, HTTPException

from shop_auth.middleware import get_current_user
from shop_auth.models import UserProfile

logger = logging.getLogger("shop.api.routes")

router = APIRouter()


@router.get("/products")
async def list_products(page: int = 1, limit: int = 20):
    logger.info("Listing products: page=%d, limit=%d", page, limit)
    return {"products": [], "page": page, "limit": limit}


@router.get("/products/{product_id}")
async def get_product(product_id: str):
    logger.info("Getting product: id=%s", product_id)
    return {"product": None}


@router.post("/products", status_code=201)
async def create_product(
    data: dict,
    current_user: UserProfile = Depends(get_current_user),
):
    logger.info(
        "Creating product: name=%s, user=%s",
        data.get("name"),
        current_user.email,
    )

    async with httpx.AsyncClient() as client:
        await client.post(
            "https://search.internal.example.com/api/v1/index",
            json={"type": "product", "data": data},
        )

    return {"product": {**data, "id": "new-id"}}


@router.put("/products/{product_id}")
async def update_product(
    product_id: str,
    data: dict,
    current_user: UserProfile = Depends(get_current_user),
):
    logger.info("Updating product: id=%s, user=%s", product_id, current_user.email)
    return {"updated": True}


@router.delete("/products/{product_id}")
async def delete_product(
    product_id: str,
    current_user: UserProfile = Depends(get_current_user),
):
    logger.info("Deleting product: id=%s, user=%s", product_id, current_user.email)
    return {"deleted": True}


@router.get("/orders")
async def list_orders(current_user: UserProfile = Depends(get_current_user)):
    logger.info("Listing orders for user=%s", current_user.id)
    return {"orders": []}


@router.post("/orders", status_code=201)
async def create_order(
    data: dict,
    current_user: UserProfile = Depends(get_current_user),
):
    logger.info("Creating order for user=%s, email=%s", current_user.id, current_user.email)

    async with httpx.AsyncClient() as client:
        resp = await client.post(
            "https://payments.internal.example.com/api/v1/charge",
            json={"user_id": current_user.id, "items": data.get("items", [])},
        )
        logger.info("Payment response: status=%d", resp.status_code)

    return {"order": {"id": "new-order", "status": "processing"}}
