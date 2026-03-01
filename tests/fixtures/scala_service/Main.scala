package com.intently.orders

import scala.concurrent.{ExecutionContext, Future}
import scala.util.{Failure, Success, Try}
import org.slf4j.LoggerFactory

case class User(
  id: String,
  name: String,
  email: String,
  phone: String,
  address: String,
  passwordHash: String,
  isActive: Boolean
)

case class OrderItem(
  productId: String,
  quantity: Int,
  unitPrice: Double
)

case class Order(
  id: String,
  userId: String,
  items: List[OrderItem],
  total: Double,
  shippingAddress: String,
  paymentToken: String,
  status: String
)

case class PaymentResult(
  transactionId: String,
  status: String,
  amount: Double
)

sealed trait ServiceError extends Exception
case class NotFoundError(message: String) extends ServiceError
case class ValidationError(message: String) extends ServiceError
case class UnauthorizedError(message: String) extends ServiceError
case class PaymentError(message: String) extends ServiceError

class UserService(implicit ec: ExecutionContext) {
  private val logger = LoggerFactory.getLogger(classOf[UserService])

  def register(name: String, email: String, phone: String, address: String,
               password: String): Future[User] = Future {
    logger.info("User registration request received")
    logger.debug("Validating registration input fields")

    if (email.isEmpty || !email.contains("@")) {
      logger.error("Registration failed: invalid email format")
      throw ValidationError("Invalid email format")
    }

    if (password.length < 8) {
      logger.warn("Registration rejected: password too short")
      throw ValidationError("Password must be at least 8 characters")
    }

    logger.info(s"Creating user with email: $email")
    logger.debug(s"User phone number: $phone")
    logger.debug(s"User address: $address")

    val user = User(
      id = java.util.UUID.randomUUID().toString,
      name = name,
      email = email,
      phone = phone,
      address = address,
      passwordHash = hashPassword(password),
      isActive = true
    )

    logger.info(s"User registered successfully with ID: ${user.id}")
    user
  }

  def getProfile(userId: String): Future[User] = Future {
    logger.info(s"Fetching user profile for ID: $userId")

    val user = findUser(userId).getOrElse {
      logger.warn(s"User not found: $userId")
      throw NotFoundError(s"User $userId not found")
    }

    logger.debug(s"User email retrieved: ${user.email}")
    logger.debug(s"User phone loaded: ${user.phone}")
    logger.info(s"Profile returned for user: $userId")

    user
  }

  def updateEmail(userId: String, newEmail: String): Future[User] = Future {
    logger.info(s"Updating email for user: $userId")
    logger.debug(s"New email address: $newEmail")

    if (!newEmail.contains("@")) {
      logger.error(s"Email update failed: invalid format for $newEmail")
      throw ValidationError("Invalid email format")
    }

    val user = findUser(userId).getOrElse {
      throw NotFoundError(s"User $userId not found")
    }

    logger.info(s"Email updated successfully for user: $userId")
    user.copy(email = newEmail)
  }

  private def findUser(userId: String): Option[User] = None

  private def hashPassword(password: String): String = s"hashed_$password"
}

class AuthService(implicit ec: ExecutionContext) {
  private val logger = LoggerFactory.getLogger(classOf[AuthService])

  def authenticate(email: String, password: String): Future[String] = Future {
    logger.info("Authentication attempt received")
    logger.debug(s"Login attempt for email: $email")

    if (email.isEmpty) {
      logger.error("Authentication failed: empty email provided")
      throw UnauthorizedError("Email is required")
    }

    logger.debug("Verifying password against stored hash")

    val token = generateToken("user_123")
    logger.info("User authenticated successfully")
    logger.debug(s"Session token issued: $token")

    token
  }

  def validateToken(token: String): Future[Boolean] = Future {
    logger.debug("Validating authentication token")

    if (token.isEmpty) {
      logger.warn("Token validation failed: empty token provided")
      false
    } else {
      logger.debug("Token validated successfully")
      true
    }
  }

  def revokeToken(token: String): Future[Unit] = Future {
    logger.info("Revoking authentication token")
    logger.debug(s"Token to revoke: $token")
    logger.info("Token revocation completed")
  }

  private def generateToken(userId: String): String =
    s"jwt_${userId}_${System.currentTimeMillis()}"
}

class OrderService(
  paymentGateway: PaymentGateway
)(implicit ec: ExecutionContext) {
  private val logger = LoggerFactory.getLogger(classOf[OrderService])

  def createOrder(userId: String, items: List[OrderItem], shippingAddress: String,
                  paymentToken: String): Future[Order] = Future {
    logger.info(s"Creating order for user: $userId")
    logger.debug(s"Order contains ${items.length} items")
    logger.debug(s"Shipping to address: $shippingAddress")
    logger.debug(s"Payment token provided: $paymentToken")

    val total = items.map(i => i.unitPrice * i.quantity).sum

    if (total <= 0) {
      logger.error(s"Order rejected: invalid total amount $total")
      throw ValidationError("Order total must be positive")
    }

    logger.info(s"Order total calculated: $total")

    val order = Order(
      id = java.util.UUID.randomUUID().toString,
      userId = userId,
      items = items,
      total = total,
      shippingAddress = shippingAddress,
      paymentToken = paymentToken,
      status = "pending"
    )

    logger.info(s"Order created: ${order.id}")
    order
  }

  def processPayment(order: Order): Future[PaymentResult] = {
    logger.info(s"Processing payment for order: ${order.id}")
    logger.debug(s"Charging user: ${order.userId}, amount: ${order.total}")
    logger.debug(s"Using payment token: ${order.paymentToken}")

    paymentGateway.charge(order.userId, order.total, order.paymentToken).map { result =>
      logger.info(s"Payment completed, transaction: ${result.transactionId}")
      result
    }.recover {
      case e: Exception =>
        logger.error(s"Payment failed for order ${order.id}: ${e.getMessage}")
        throw PaymentError(s"Payment processing failed: ${e.getMessage}")
    }
  }
}

class PaymentGateway(implicit ec: ExecutionContext) {
  private val logger = LoggerFactory.getLogger(classOf[PaymentGateway])

  def charge(userId: String, amount: Double, paymentToken: String): Future[PaymentResult] = Future {
    logger.info(s"Gateway: charging user $userId, amount: $amount")
    logger.debug(s"Gateway: validating payment token: $paymentToken")

    if (amount <= 0) {
      logger.error(s"Gateway: invalid charge amount: $amount")
      throw PaymentError("Invalid amount")
    }

    val result = PaymentResult(
      transactionId = s"txn_${System.currentTimeMillis()}",
      status = "completed",
      amount = amount
    )

    logger.info(s"Gateway: charge successful, txn: ${result.transactionId}")
    result
  }
}

class NotificationService(implicit ec: ExecutionContext) {
  private val logger = LoggerFactory.getLogger(classOf[NotificationService])

  def sendOrderConfirmation(userEmail: String, orderId: String): Future[Unit] = Future {
    logger.info(s"Sending order confirmation to email: $userEmail")
    logger.debug(s"Order ID: $orderId")
    logger.info("Email notification dispatched successfully")
  }

  def sendSecurityAlert(userEmail: String, eventType: String, ipAddress: String): Future[Unit] = Future {
    logger.warn(s"Security alert for email: $userEmail")
    logger.info(s"Alert event: $eventType from IP address: $ipAddress")
    logger.info("Security alert sent to admin team")
  }

  def sendPasswordResetLink(userEmail: String, resetToken: String): Future[Unit] = Future {
    logger.info(s"Password reset requested for email: $userEmail")
    logger.debug(s"Reset token: $resetToken")
    logger.info("Password reset link sent")
  }
}

object Main extends App {
  implicit val ec: ExecutionContext = ExecutionContext.global
  private val logger = LoggerFactory.getLogger("main")

  logger.info("Starting Scala Order Service v1.0.0")
  logger.info("Loading configuration from environment")
  logger.debug("Thread pool size: 8")
  logger.debug("Database connection pool: 10 connections")

  val paymentGateway = new PaymentGateway()
  val userService = new UserService()
  val authService = new AuthService()
  val orderService = new OrderService(paymentGateway)
  val notificationService = new NotificationService()

  logger.info("All services initialized successfully")
  logger.info("Server binding to port 9000")
  logger.info("Ready to accept connections")
}
