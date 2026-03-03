"""API route definitions."""

import logging
from typing import List

from fastapi import APIRouter, Depends, HTTPException

from .models import User, Product
from ..config import DATABASE_URL

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/v1")


@router.get("/users", response_model=List[User])
async def list_users():
    logger.info("Listing all users")
    return []


@router.get("/users/{user_id}")
async def get_user(user_id: int):
    logger.info("Fetching user: user_id=%d", user_id)
    return None


@router.post("/users")
async def create_user(user: User):
    logger.info("Creating user: email=%s", user.email)
    return user


@router.get("/products")
async def list_products():
    return []


@router.get("/products/{product_id}")
async def get_product(product_id: int):
    return None
