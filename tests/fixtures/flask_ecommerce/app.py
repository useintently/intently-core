"""
Flask E-Commerce Application — Main Application Module

Full-featured e-commerce backend with user management, product catalog,
order processing, and payment integration. Uses Flask blueprints for
modular route organization.
"""

import logging
import time
from datetime import datetime, timedelta
from functools import wraps
from uuid import uuid4

import httpx
import requests
from flask import Flask, abort, g, jsonify, redirect, request, session, url_for
from flask_jwt_extended import (
    JWTManager,
    create_access_token,
    get_jwt_identity,
    jwt_required,
    verify_jwt_in_request,
)
from flask_login import LoginManager, current_user, login_required, login_user, logout_user
from werkzeug.security import check_password_hash, generate_password_hash

from routes.payments import payments_bp
from routes.products import products_bp
from config import Config

logger = logging.getLogger("flask_ecommerce")
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s %(message)s")

app = Flask(__name__)
app.config.from_object(Config)
app.secret_key = Config.SECRET_KEY

jwt = JWTManager(app)
login_manager = LoginManager(app)
login_manager.login_view = "login_page"

app.register_blueprint(payments_bp, url_prefix="/api/payments")
app.register_blueprint(products_bp, url_prefix="/api/products")

ANALYTICS_URL = "https://analytics.internal/api/v1/events"
EMAIL_SERVICE_URL = "https://email-service.internal/api/v1/send"
INVENTORY_SERVICE_URL = "https://inventory.internal/api/v1"


def admin_required(f):
    """Decorator that requires admin role."""

    @wraps(f)
    def decorated_function(*args, **kwargs):
        if not current_user.is_authenticated or not current_user.is_admin:
            logger.warning(
                "Admin access denied: user=%s email=%s ip_address=%s path=%s",
                getattr(current_user, "id", "anonymous"),
                getattr(current_user, "email", "unknown"),
                request.remote_addr,
                request.path,
            )
            abort(403)
        return f(*args, **kwargs)

    return decorated_function


@app.before_request
def before_request_handler():
    g.request_start_time = time.monotonic()
    g.request_id = request.headers.get("X-Request-ID", str(uuid4()))

    logger.info(
        "Request started: method=%s path=%s ip_address=%s request_id=%s",
        request.method,
        request.path,
        request.remote_addr,
        g.request_id,
    )


@app.after_request
def after_request_handler(response):
    duration = time.monotonic() - g.get("request_start_time", time.monotonic())
    logger.info(
        "Request completed: method=%s path=%s status=%d duration=%.3fs ip_address=%s",
        request.method,
        request.path,
        response.status_code,
        duration,
        request.remote_addr,
    )
    response.headers["X-Request-ID"] = g.get("request_id", "unknown")
    response.headers["X-Response-Time"] = f"{duration:.3f}s"
    return response


@app.errorhandler(404)
def not_found(error):
    logger.warning("404 Not Found: path=%s ip_address=%s", request.path, request.remote_addr)
    return jsonify({"error": "Not found"}), 404


@app.errorhandler(500)
def internal_error(error):
    logger.error("500 Internal Error: path=%s error=%s", request.path, str(error))
    return jsonify({"error": "Internal server error"}), 500


@app.route("/")
def index():
    logger.info("Index page accessed")
    return jsonify({"name": "Flask E-Commerce API", "version": "1.8.3"})


@app.route("/health")
def health_check():
    logger.info("Health check requested from ip_address=%s", request.remote_addr)
    return jsonify({"status": "healthy", "timestamp": datetime.utcnow().isoformat()})


@app.route("/health/ready")
def readiness_check():
    checks = {"database": "ok", "redis": "ok", "stripe": "ok"}
    logger.info("Readiness check: %s", checks)
    return jsonify({"status": "ready", "checks": checks})


@app.route("/api/users", methods=["GET"])
@jwt_required()
def list_users():
    """List all users. Requires JWT authentication."""
    current_identity = get_jwt_identity()
    page = request.args.get("page", 1, type=int)
    per_page = request.args.get("per_page", 50, type=int)

    logger.info(
        "User list requested: by_user=%s page=%d per_page=%d",
        current_identity,
        page,
        per_page,
    )

    users = get_all_users(page=page, per_page=per_page)
    return jsonify({"users": users, "page": page, "per_page": per_page})


@app.route("/api/users", methods=["POST"])
def create_user():
    """Register a new user. Public endpoint."""
    data = request.get_json()
    email = data.get("email")
    password = data.get("password")
    name = data.get("name")
    phone = data.get("phone")

    if not email or not password or not name:
        return jsonify({"error": "Missing required fields"}), 400

    logger.info(
        "User registration: email=%s name=%s phone=%s ip_address=%s",
        email,
        name,
        phone,
        request.remote_addr,
    )

    existing = find_user_by_email(email)
    if existing:
        logger.warning("Registration failed — duplicate email: email=%s", email)
        return jsonify({"error": "Email already registered"}), 409

    hashed = generate_password_hash(password)
    user = save_user(email=email, password_hash=hashed, name=name, phone=phone)

    logger.info("User created: user_id=%s email=%s name=%s", user["id"], email, name)

    try:
        requests.post(
            EMAIL_SERVICE_URL,
            json={"to": email, "template": "welcome", "data": {"name": name}},
            timeout=10,
        )
        logger.info("Welcome email sent to email=%s", email)
    except requests.RequestException as e:
        logger.error("Failed to send welcome email to %s: %s", email, str(e))

    return jsonify(user), 201


@app.route("/api/users/<int:user_id>", methods=["GET"])
@jwt_required()
def get_user(user_id):
    """Get user by ID. Requires JWT authentication."""
    current_identity = get_jwt_identity()
    user = find_user_by_id(user_id)
    if not user:
        return jsonify({"error": "User not found"}), 404

    logger.info("User fetched: user_id=%d by_user=%s", user_id, current_identity)
    return jsonify(user)


@app.route("/api/users/<int:user_id>", methods=["PUT"])
@jwt_required()
def update_user(user_id):
    """Update user profile. Users can only update their own profile."""
    current_identity = get_jwt_identity()
    data = request.get_json()

    if str(user_id) != current_identity:
        logger.warning(
            "Unauthorized update: user=%s tried to update user_id=%d email=%s",
            current_identity,
            user_id,
            data.get("email", "unknown"),
        )
        return jsonify({"error": "Access denied"}), 403

    user = update_user_in_db(user_id, data)
    logger.info("User updated: user_id=%d fields=%s", user_id, list(data.keys()))
    return jsonify(user)


@app.route("/api/users/<int:user_id>", methods=["DELETE"])
@jwt_required()
@admin_required
def delete_user(user_id):
    """Delete a user. Admin only."""
    user = find_user_by_id(user_id)
    if not user:
        return jsonify({"error": "User not found"}), 404

    soft_delete_user(user_id)
    logger.info(
        "User deleted: user_id=%d email=%s deleted_by=%s",
        user_id,
        user.get("email"),
        get_jwt_identity(),
    )
    return "", 204


@app.route("/api/users/login", methods=["POST"])
def user_login():
    """Authenticate and return JWT token."""
    data = request.get_json()
    email = data.get("email")
    password = data.get("password")

    logger.info("Login attempt: email=%s ip_address=%s", email, request.remote_addr)

    user = find_user_by_email(email)
    if not user or not check_password_hash(user["password_hash"], password):
        logger.warning(
            "Login failed: email=%s ip_address=%s",
            email,
            request.remote_addr,
        )
        return jsonify({"error": "Invalid credentials"}), 401

    access_token = create_access_token(identity=str(user["id"]))

    logger.info(
        "Login successful: user_id=%s email=%s ip_address=%s",
        user["id"],
        email,
        request.remote_addr,
    )

    try:
        httpx.post(
            ANALYTICS_URL,
            json={
                "event": "login",
                "user_id": user["id"],
                "email": email,
                "ip_address": request.remote_addr,
            },
            timeout=5,
        )
    except httpx.HTTPError:
        pass

    return jsonify({"access_token": access_token, "token_type": "bearer"})


@app.route("/api/products", methods=["GET"])
def list_products():
    """List all products. Public endpoint."""
    category = request.args.get("category")
    min_price = request.args.get("min_price", type=float)
    max_price = request.args.get("max_price", type=float)
    search = request.args.get("q")

    logger.info(
        "Product listing: category=%s min_price=%s max_price=%s search=%s",
        category,
        min_price,
        max_price,
        search,
    )

    try:
        response = requests.get(
            f"{INVENTORY_SERVICE_URL}/products",
            params={
                "category": category,
                "min_price": min_price,
                "max_price": max_price,
                "search": search,
            },
            timeout=10,
        )
        response.raise_for_status()
        products = response.json()
    except requests.RequestException as e:
        logger.error("Inventory service error: %s", str(e))
        return jsonify({"error": "Product catalog temporarily unavailable"}), 503

    return jsonify({"products": products})


@app.route("/api/orders", methods=["POST"])
@jwt_required()
def create_order():
    """Create a new order. Requires JWT authentication."""
    current_identity = get_jwt_identity()
    data = request.get_json()
    items = data.get("items", [])
    shipping_address = data.get("shipping_address")

    if not items:
        return jsonify({"error": "Order must contain at least one item"}), 400

    logger.info(
        "Order creation: user_id=%s item_count=%d shipping=%s",
        current_identity,
        len(items),
        shipping_address,
    )

    try:
        stock_check = requests.post(
            f"{INVENTORY_SERVICE_URL}/stock/check",
            json={"items": items},
            timeout=10,
        )
        stock_check.raise_for_status()
        stock_result = stock_check.json()
    except requests.RequestException as e:
        logger.error("Stock check failed: %s", str(e))
        return jsonify({"error": "Could not verify stock availability"}), 503

    if not stock_result.get("all_available"):
        logger.warning(
            "Order failed — out of stock: user_id=%s items=%s",
            current_identity,
            stock_result.get("unavailable_items"),
        )
        return jsonify({
            "error": "Some items are out of stock",
            "unavailable": stock_result.get("unavailable_items"),
        }), 409

    order = save_order(user_id=current_identity, items=items, shipping_address=shipping_address)

    logger.info(
        "Order created: order_id=%s user_id=%s total=%s",
        order["id"],
        current_identity,
        order["total"],
    )

    try:
        httpx.post(
            ANALYTICS_URL,
            json={
                "event": "order_created",
                "order_id": order["id"],
                "user_id": current_identity,
                "total": order["total"],
                "item_count": len(items),
            },
            timeout=5,
        )
    except httpx.HTTPError:
        pass

    return jsonify(order), 201


@app.route("/api/orders/<int:order_id>", methods=["GET"])
@login_required
def get_order(order_id):
    """Get order details. Login required."""
    order = find_order_by_id(order_id)
    if not order:
        return jsonify({"error": "Order not found"}), 404

    if order["user_id"] != current_user.id and not current_user.is_admin:
        logger.warning(
            "Unauthorized order access: user=%s tried to view order=%d",
            current_user.email,
            order_id,
        )
        return jsonify({"error": "Access denied"}), 403

    logger.info("Order fetched: order_id=%d by_user=%s", order_id, current_user.email)
    return jsonify(order)


@app.route("/admin/dashboard")
@login_required
@admin_required
def admin_dashboard():
    """Admin dashboard. Login and admin role required."""
    logger.info("Admin dashboard accessed: user=%s email=%s", current_user.id, current_user.email)
    stats = get_dashboard_stats()
    return jsonify(stats)


@app.route("/admin/users")
@login_required
@admin_required
def admin_users():
    """Admin user management page."""
    logger.info("Admin user list accessed by %s", current_user.email)
    users = get_all_users(page=1, per_page=100)
    return jsonify({"users": users})


@app.route("/api/search", methods=["GET"])
def search():
    """Global search endpoint. Public."""
    query = request.args.get("q", "")
    category = request.args.get("type", "all")

    if len(query) < 2:
        return jsonify({"error": "Search query must be at least 2 characters"}), 400

    logger.info("Search: query=%s type=%s ip_address=%s", query, category, request.remote_addr)

    try:
        response = httpx.get(
            "https://search.internal/api/v1/search",
            params={"q": query, "type": category},
            timeout=10,
        )
        results = response.json()
    except httpx.HTTPError as e:
        logger.error("Search service error: %s", str(e))
        return jsonify({"error": "Search temporarily unavailable"}), 503

    return jsonify(results)


@app.route("/api/webhooks/stripe", methods=["POST"])
def stripe_webhook():
    """Handle Stripe webhook events."""
    payload = request.get_data()
    sig_header = request.headers.get("Stripe-Signature")

    logger.info("Stripe webhook received: sig=%s", sig_header[:20] if sig_header else "missing")

    try:
        event = process_stripe_webhook(payload, sig_header)
    except ValueError as e:
        logger.error("Invalid Stripe webhook payload: %s", str(e))
        return jsonify({"error": "Invalid payload"}), 400

    event_type = event.get("type")
    if event_type == "payment_intent.succeeded":
        email = event["data"]["object"]["metadata"].get("email")
        logger.info("Payment succeeded: email=%s", email)
    elif event_type == "charge.refunded":
        logger.info("Refund processed via webhook")

    return jsonify({"received": True}), 200


def get_all_users(**kwargs):
    return []


def find_user_by_email(email):
    return None


def find_user_by_id(user_id):
    return None


def save_user(**kwargs):
    return {"id": 1}


def update_user_in_db(user_id, data):
    return {}


def soft_delete_user(user_id):
    pass


def save_order(**kwargs):
    return {"id": 1, "total": 0}


def find_order_by_id(order_id):
    return None


def get_dashboard_stats():
    return {}


def process_stripe_webhook(payload, sig):
    return {}
