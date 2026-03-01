"""
Authentication Dependencies — FastAPI E-Commerce Platform

Provides JWT-based authentication, role-based authorization,
and reusable security dependencies for route protection.
"""

import logging
from datetime import datetime, timedelta
from functools import wraps
from typing import Annotated, Optional
from uuid import UUID

import httpx
import requests
from fastapi import Depends, HTTPException, Request, status
from fastapi.security import OAuth2PasswordBearer
from jose import JWTError, jwt
from passlib.context import CryptContext
from pydantic import BaseModel

log = logging.getLogger("ecommerce.auth")

SECRET_KEY = "change-me-in-production"
ALGORITHM = "HS256"
ACCESS_TOKEN_EXPIRE_MINUTES = 30

oauth2_scheme = OAuth2PasswordBearer(tokenUrl="/api/v1/users/login")
pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

IDENTITY_SERVICE_URL = "https://identity.internal/api/v1"
AUDIT_SERVICE_URL = "https://audit-log.internal/api/v1/events"


class TokenData(BaseModel):
    user_id: Optional[UUID] = None
    email: Optional[str] = None


class User(BaseModel):
    id: UUID
    email: str
    name: str
    phone: Optional[str] = None
    hashed_password: str
    is_active: bool = True
    is_admin: bool = False
    is_verified: bool = False
    last_login: Optional[datetime] = None


def verify_password(plain_password: str, hashed_password: str) -> bool:
    """Verify a password against its hash."""
    return pwd_context.verify(plain_password, hashed_password)


def get_password_hash(password: str) -> str:
    """Hash a password using bcrypt."""
    return pwd_context.hash(password)


def create_access_token(data: dict, expires_delta: Optional[timedelta] = None) -> str:
    """Create a JWT access token."""
    to_encode = data.copy()
    expire = datetime.utcnow() + (expires_delta or timedelta(minutes=ACCESS_TOKEN_EXPIRE_MINUTES))
    to_encode.update({"exp": expire, "iat": datetime.utcnow()})

    encoded_jwt = jwt.encode(to_encode, SECRET_KEY, algorithm=ALGORITHM)

    log.info(
        "Access token created: email=%s expires=%s",
        data.get("email", "unknown"),
        expire.isoformat(),
    )
    return encoded_jwt


def create_refresh_token(user_id: str, email: str) -> str:
    """Create a long-lived refresh token."""
    expire = datetime.utcnow() + timedelta(days=7)
    data = {
        "sub": user_id,
        "email": email,
        "type": "refresh",
        "exp": expire,
        "iat": datetime.utcnow(),
    }

    log.info("Refresh token created: user_id=%s email=%s", user_id, email)
    return jwt.encode(data, SECRET_KEY, algorithm=ALGORITHM)


async def get_current_user(
    request: Request,
    token: Annotated[str, Depends(oauth2_scheme)],
) -> User:
    """Decode JWT and retrieve the current user. Core auth dependency."""
    client_ip = request.client.host if request.client else "unknown"

    credentials_exception = HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail="Could not validate credentials",
        headers={"WWW-Authenticate": "Bearer"},
    )

    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        user_id = payload.get("sub")
        email = payload.get("email")

        if user_id is None:
            log.warning("Token missing user_id: ip_address=%s", client_ip)
            raise credentials_exception

        token_data = TokenData(user_id=UUID(user_id), email=email)
    except JWTError as e:
        log.warning(
            "JWT decode failed: error=%s ip_address=%s token_prefix=%s",
            str(e),
            client_ip,
            token[:20] if token else "empty",
        )
        raise credentials_exception

    user = await get_user_from_db(token_data.user_id)
    if user is None:
        log.warning(
            "User not found for valid token: user_id=%s email=%s ip_address=%s",
            token_data.user_id,
            token_data.email,
            client_ip,
        )
        raise credentials_exception

    log.info(
        "User authenticated: user_id=%s email=%s ip_address=%s",
        user.id,
        user.email,
        client_ip,
    )

    try:
        httpx.post(
            AUDIT_SERVICE_URL,
            json={
                "event": "user_authenticated",
                "user_id": str(user.id),
                "email": user.email,
                "ip_address": client_ip,
                "timestamp": datetime.utcnow().isoformat(),
            },
            timeout=5,
        )
    except httpx.HTTPError:
        pass

    return user


async def get_current_active_user(
    current_user: Annotated[User, Depends(get_current_user)],
) -> User:
    """Ensure the authenticated user's account is active."""
    if not current_user.is_active:
        log.warning(
            "Inactive user attempted access: user_id=%s email=%s",
            current_user.id,
            current_user.email,
        )
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Account is inactive. Please contact support.",
        )
    return current_user


async def get_current_admin_user(
    current_user: Annotated[User, Depends(get_current_active_user)],
) -> User:
    """Ensure the authenticated user has admin privileges."""
    if not current_user.is_admin:
        log.warning(
            "Non-admin user attempted admin action: user_id=%s email=%s name=%s",
            current_user.id,
            current_user.email,
            current_user.name,
        )
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Admin access required",
        )

    log.info(
        "Admin access granted: user_id=%s email=%s",
        current_user.id,
        current_user.email,
    )
    return current_user


def login_required(func):
    """Decorator that ensures the user is authenticated before proceeding."""

    @wraps(func)
    async def wrapper(*args, request: Request = None, **kwargs):
        token = None
        if request:
            auth_header = request.headers.get("Authorization", "")
            if auth_header.startswith("Bearer "):
                token = auth_header[7:]

        if not token:
            client_ip = request.client.host if request and request.client else "unknown"
            log.warning("Unauthenticated request to protected endpoint: ip_address=%s", client_ip)
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Authentication required",
            )

        try:
            payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
            user_id = payload.get("sub")
            email = payload.get("email")
        except JWTError as e:
            log.warning("Invalid token in login_required: error=%s", str(e))
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid authentication token",
            )

        log.info("login_required passed: user_id=%s email=%s", user_id, email)
        return await func(*args, request=request, **kwargs)

    return wrapper


def require_permissions(*permissions: str):
    """Dependency factory that checks for specific permissions."""

    async def check_permissions(
        current_user: Annotated[User, Depends(get_current_active_user)],
    ) -> User:
        try:
            response = requests.get(
                f"{IDENTITY_SERVICE_URL}/users/{current_user.id}/permissions",
                timeout=5,
            )
            response.raise_for_status()
            user_permissions = set(response.json().get("permissions", []))
        except requests.RequestException as e:
            log.error(
                "Permission check failed: user_id=%s email=%s error=%s",
                current_user.id,
                current_user.email,
                str(e),
            )
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail="Permission service unavailable",
            )

        missing = set(permissions) - user_permissions
        if missing:
            log.warning(
                "Insufficient permissions: user_id=%s email=%s required=%s missing=%s",
                current_user.id,
                current_user.email,
                permissions,
                missing,
            )
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Missing permissions: {', '.join(missing)}",
            )

        log.info(
            "Permission check passed: user_id=%s email=%s permissions=%s",
            current_user.id,
            current_user.email,
            permissions,
        )
        return current_user

    return Depends(check_permissions)


async def validate_api_key(request: Request) -> str:
    """Validate an API key from the X-API-Key header for service-to-service auth."""
    api_key = request.headers.get("X-API-Key")
    client_ip = request.client.host if request.client else "unknown"

    if not api_key:
        log.warning("Missing API key: ip_address=%s path=%s", client_ip, request.url.path)
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="API key required",
        )

    try:
        response = requests.post(
            f"{IDENTITY_SERVICE_URL}/api-keys/validate",
            json={"key": api_key},
            timeout=5,
        )
        response.raise_for_status()
        key_data = response.json()
    except requests.RequestException as e:
        log.error("API key validation service error: ip_address=%s error=%s", client_ip, str(e))
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Authentication service unavailable",
        )

    if not key_data.get("valid"):
        log.warning(
            "Invalid API key: ip_address=%s key_prefix=%s",
            client_ip,
            api_key[:8] if api_key else "empty",
        )
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid API key",
        )

    log.info(
        "API key validated: service=%s ip_address=%s",
        key_data.get("service_name", "unknown"),
        client_ip,
    )
    return key_data.get("service_name", "unknown")


async def get_user_from_db(user_id: UUID) -> Optional[User]:
    """Stub: retrieve user from database."""
    return None
