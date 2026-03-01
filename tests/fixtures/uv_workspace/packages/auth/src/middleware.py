"""
Authentication middleware for the shop platform.
"""

import logging

import httpx
from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer

from .models import UserProfile

logger = logging.getLogger("shop.auth")

security = HTTPBearer()


async def get_current_user(
    credentials: HTTPAuthorizationCredentials = Depends(security),
) -> UserProfile:
    """Validate JWT token and return the current user profile."""
    token = credentials.credentials

    async with httpx.AsyncClient() as client:
        resp = await client.post(
            "https://auth.internal.example.com/api/v1/verify",
            json={"token": token},
        )

    if resp.status_code != 200:
        logger.warning(
            "Token validation failed: status=%d, token_prefix=%s",
            resp.status_code,
            token[:8],
        )
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid or expired token",
        )

    data = resp.json()
    logger.info("Token validated: user_id=%s, email=%s", data["user_id"], data["email"])

    return UserProfile(
        id=data["user_id"],
        email=data["email"],
        name=data["name"],
        role=data.get("role", "customer"),
    )


async def require_admin(
    current_user: UserProfile = Depends(get_current_user),
) -> UserProfile:
    """Ensure the current user has admin privileges."""
    if current_user.role != "admin":
        logger.warning(
            "Admin access denied: user_id=%s, role=%s",
            current_user.id,
            current_user.role,
        )
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Admin access required",
        )
    return current_user
