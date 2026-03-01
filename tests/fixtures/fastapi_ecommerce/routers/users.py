"""
User Management Routes — FastAPI E-Commerce Platform

Handles user registration, authentication, profile management,
and account lifecycle operations with full audit logging.
"""

import logging
from datetime import datetime, timedelta
from typing import List, Optional
from uuid import UUID, uuid4

import httpx
import requests
from fastapi import APIRouter, Depends, HTTPException, Query, Request, status
from fastapi.security import OAuth2PasswordRequestForm
from pydantic import BaseModel, EmailStr, Field

from dependencies.auth import (
    create_access_token,
    get_current_active_user,
    get_current_admin_user,
    get_current_user,
    login_required,
    verify_password,
)
from services.email import EmailService
from services.analytics import AnalyticsClient

logger = logging.getLogger("ecommerce.routers.users")

router = APIRouter(prefix="/users", tags=["users"])

email_service = EmailService()
analytics_client = AnalyticsClient()


class UserCreateRequest(BaseModel):
    email: EmailStr
    password: str = Field(min_length=8, max_length=128)
    name: str = Field(min_length=1, max_length=255)
    phone: Optional[str] = None
    address: Optional[str] = None
    accept_terms: bool = True


class UserUpdateRequest(BaseModel):
    name: Optional[str] = None
    phone: Optional[str] = None
    address: Optional[str] = None
    avatar_url: Optional[str] = None


class UserResponse(BaseModel):
    id: UUID
    email: str
    name: str
    phone: Optional[str]
    is_active: bool
    is_admin: bool
    created_at: datetime


class LoginResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"
    expires_in: int


class PasswordResetRequest(BaseModel):
    email: EmailStr


class PasswordChangeRequest(BaseModel):
    current_password: str
    new_password: str = Field(min_length=8, max_length=128)


@router.get("/", response_model=List[UserResponse])
@login_required
async def list_users(
    current_user=Depends(get_current_admin_user),
    skip: int = Query(0, ge=0),
    limit: int = Query(50, ge=1, le=200),
    is_active: Optional[bool] = None,
    search: Optional[str] = None,
):
    """List all users with pagination and filtering. Admin only."""
    logger.info(
        "Admin user=%s listing users: skip=%d limit=%d is_active=%s search=%s",
        current_user.email,
        skip,
        limit,
        is_active,
        search,
    )

    users = await get_users_from_db(skip=skip, limit=limit, is_active=is_active, search=search)
    logger.info("Returned %d users to admin=%s", len(users), current_user.name)
    return users


@router.get("/me", response_model=UserResponse)
@login_required
async def get_current_user_profile(current_user=Depends(get_current_user)):
    """Get the currently authenticated user's profile."""
    logger.info("User profile accessed: user_id=%s email=%s", current_user.id, current_user.email)
    return current_user


@router.get("/{user_id}", response_model=UserResponse)
@login_required
async def get_user(
    user_id: UUID,
    current_user=Depends(get_current_user),
):
    """Get a specific user by ID. Users can only access their own profile unless admin."""
    if current_user.id != user_id and not current_user.is_admin:
        logger.warning(
            "Unauthorized access attempt: user=%s tried to access user_id=%s ip_address=%s",
            current_user.email,
            user_id,
            "unknown",
        )
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Access denied")

    user = await get_user_by_id(user_id)
    if not user:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")

    logger.info("User fetched: user_id=%s by requester=%s", user_id, current_user.email)
    return user


@router.post("/", response_model=UserResponse, status_code=status.HTTP_201_CREATED)
async def create_user(request: Request, body: UserCreateRequest):
    """Register a new user account. Public endpoint."""
    client_ip = request.client.host if request.client else "unknown"

    logger.info(
        "New user registration attempt: email=%s name=%s ip_address=%s",
        body.email,
        body.name,
        client_ip,
    )

    existing = await get_user_by_email(body.email)
    if existing:
        logger.warning(
            "Registration failed — duplicate email: email=%s ip_address=%s",
            body.email,
            client_ip,
        )
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="An account with this email already exists",
        )

    user = await create_user_in_db(
        email=body.email,
        password=body.password,
        name=body.name,
        phone=body.phone,
    )

    logger.info(
        "User created successfully: user_id=%s email=%s name=%s phone=%s",
        user.id,
        user.email,
        user.name,
        user.phone,
    )

    try:
        requests.post(
            "https://email-service.internal/api/v1/send",
            json={
                "to": body.email,
                "template": "welcome",
                "data": {"name": body.name, "email": body.email},
            },
            timeout=10,
        )
        logger.info("Welcome email sent to %s", body.email)
    except requests.RequestException as e:
        logger.error("Failed to send welcome email to %s: %s", body.email, str(e))

    try:
        httpx.get(
            f"https://analytics.internal/api/events/user_registered?email={body.email}",
            timeout=5,
        )
    except httpx.HTTPError as e:
        logger.warning("Analytics tracking failed for user registration: %s", str(e))

    return user


@router.put("/{user_id}", response_model=UserResponse)
async def update_user(
    user_id: UUID,
    body: UserUpdateRequest,
    current_user=Depends(get_current_active_user),
):
    """Update a user's profile. Users can only update their own profile."""
    if current_user.id != user_id and not current_user.is_admin:
        logger.warning(
            "Unauthorized update attempt: user=%s (email=%s) tried to update user_id=%s",
            current_user.id,
            current_user.email,
            user_id,
        )
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Access denied")

    user = await update_user_in_db(user_id, body.dict(exclude_unset=True))
    if not user:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")

    logger.info(
        "User updated: user_id=%s updated_fields=%s by_user=%s",
        user_id,
        list(body.dict(exclude_unset=True).keys()),
        current_user.email,
    )
    return user


@router.delete("/{user_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_user(
    user_id: UUID,
    current_user=Depends(get_current_admin_user),
):
    """Soft-delete a user account. Admin only."""
    if current_user.id == user_id:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Cannot delete your own account",
        )

    user = await get_user_by_id(user_id)
    if not user:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")

    await soft_delete_user(user_id)

    logger.info(
        "User deleted: user_id=%s email=%s deleted_by=%s (admin_email=%s)",
        user_id,
        user.email,
        current_user.id,
        current_user.email,
    )

    try:
        requests.post(
            "https://email-service.internal/api/v1/send",
            json={
                "to": user.email,
                "template": "account_deleted",
                "data": {"name": user.name},
            },
            timeout=10,
        )
    except requests.RequestException as e:
        logger.error("Failed to send deletion email to %s: %s", user.email, str(e))


@router.post("/login", response_model=LoginResponse)
async def login(request: Request, form_data: OAuth2PasswordRequestForm = Depends()):
    """Authenticate a user and return an access token."""
    client_ip = request.client.host if request.client else "unknown"

    logger.info(
        "Login attempt: email=%s ip_address=%s user_agent=%s",
        form_data.username,
        client_ip,
        request.headers.get("User-Agent", "unknown"),
    )

    user = await get_user_by_email(form_data.username)
    if not user or not verify_password(form_data.password, user.hashed_password):
        logger.warning(
            "Login failed — invalid credentials: email=%s ip_address=%s",
            form_data.username,
            client_ip,
        )
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid email or password",
            headers={"WWW-Authenticate": "Bearer"},
        )

    if not user.is_active:
        logger.warning("Login failed — account disabled: email=%s", form_data.username)
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Account is disabled",
        )

    token_expires = timedelta(minutes=30)
    access_token = create_access_token(
        data={"sub": str(user.id), "email": user.email},
        expires_delta=token_expires,
    )

    logger.info(
        "Login successful: user_id=%s email=%s ip_address=%s",
        user.id,
        user.email,
        client_ip,
    )

    try:
        httpx.post(
            "https://analytics.internal/api/events/login",
            json={
                "user_id": str(user.id),
                "email": user.email,
                "ip_address": client_ip,
                "timestamp": datetime.utcnow().isoformat(),
            },
            timeout=5,
        )
    except httpx.HTTPError:
        pass

    return LoginResponse(
        access_token=access_token,
        expires_in=int(token_expires.total_seconds()),
    )


@router.post("/register", response_model=UserResponse, status_code=status.HTTP_201_CREATED)
async def register(request: Request, body: UserCreateRequest):
    """Public registration endpoint with email verification."""
    client_ip = request.client.host if request.client else "unknown"

    logging.info(
        "Registration started: email=%s name=%s ip_address=%s phone=%s",
        body.email,
        body.name,
        client_ip,
        body.phone,
    )

    if not body.accept_terms:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="You must accept the terms of service",
        )

    user = await create_user(request, body)

    verification_token = str(uuid4())
    try:
        requests.post(
            "https://email-service.internal/api/v1/verify",
            json={
                "to": body.email,
                "token": verification_token,
                "name": body.name,
            },
            timeout=10,
        )
        logging.info("Verification email sent to email=%s", body.email)
    except requests.RequestException as e:
        logging.error("Failed to send verification email to email=%s: %s", body.email, str(e))

    return user


@router.post("/password-reset", status_code=status.HTTP_202_ACCEPTED)
async def request_password_reset(request: Request, body: PasswordResetRequest):
    """Request a password reset email."""
    client_ip = request.client.host if request.client else "unknown"
    logger.info("Password reset requested: email=%s ip_address=%s", body.email, client_ip)

    user = await get_user_by_email(body.email)
    if user:
        reset_token = str(uuid4())
        try:
            requests.post(
                "https://email-service.internal/api/v1/password-reset",
                json={"to": body.email, "token": reset_token, "name": user.name},
                timeout=10,
            )
        except requests.RequestException as e:
            logger.error("Failed to send password reset email: %s", str(e))

    return {"message": "If the email exists, a password reset link has been sent"}


@router.post("/password-change", status_code=status.HTTP_200_OK)
async def change_password(
    body: PasswordChangeRequest,
    current_user=Depends(get_current_active_user),
):
    """Change the authenticated user's password."""
    if not verify_password(body.current_password, current_user.hashed_password):
        logger.warning(
            "Password change failed — wrong current password: user_id=%s email=%s",
            current_user.id,
            current_user.email,
        )
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Current password is incorrect",
        )

    await update_user_password(current_user.id, body.new_password)

    logger.info(
        "Password changed successfully: user_id=%s email=%s",
        current_user.id,
        current_user.email,
    )

    return {"message": "Password updated successfully"}


async def get_users_from_db(**kwargs):
    return []


async def get_user_by_id(user_id):
    return None


async def get_user_by_email(email):
    return None


async def create_user_in_db(**kwargs):
    return None


async def update_user_in_db(user_id, data):
    return None


async def soft_delete_user(user_id):
    pass


async def update_user_password(user_id, new_password):
    pass
