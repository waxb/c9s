import UIKit
import UserNotifications
import UserNotificationsUI

/// Rich notification content extension that shows implementation details
/// inline when the user long-presses or expands a notification.
///
/// Setup: This file is part of a Notification Content Extension target.
/// To use it, add a new "Notification Content Extension" target in Xcode
/// and move this file to that target. Configure Info.plist with:
///   - NSExtensionPointIdentifier: com.apple.usernotifications.content-extension
///   - UNNotificationExtensionCategory: array of category IDs
///   - UNNotificationExtensionInitialContentSizeRatio: 0.5
class NotificationViewController: UIViewController, UNNotificationContentExtension {

    private let titleLabel = UILabel()
    private let statusLabel = UILabel()
    private let messageLabel = UILabel()
    private let categoryIcon = UIImageView()

    override func viewDidLoad() {
        super.viewDidLoad()
        setupUI()
    }

    func didReceive(_ notification: UNNotification) {
        let content = notification.request.content
        let userInfo = content.userInfo

        titleLabel.text = userInfo["title"] as? String ?? content.title
        messageLabel.text = userInfo["message"] as? String ?? content.body

        let category = userInfo["category"] as? String ?? ""
        configureForCategory(category)
    }

    // MARK: - UI Setup

    private func setupUI() {
        view.backgroundColor = .systemBackground

        // Category icon
        categoryIcon.translatesAutoresizingMaskIntoConstraints = false
        categoryIcon.contentMode = .scaleAspectFit
        categoryIcon.tintColor = .label
        view.addSubview(categoryIcon)

        // Title
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.font = .preferredFont(forTextStyle: .headline)
        titleLabel.numberOfLines = 2
        view.addSubview(titleLabel)

        // Status pill
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.font = .preferredFont(forTextStyle: .caption1)
        statusLabel.textAlignment = .center
        statusLabel.layer.cornerRadius = 8
        statusLabel.layer.masksToBounds = true
        statusLabel.textColor = .white
        view.addSubview(statusLabel)

        // Message
        messageLabel.translatesAutoresizingMaskIntoConstraints = false
        messageLabel.font = .preferredFont(forTextStyle: .body)
        messageLabel.textColor = .secondaryLabel
        messageLabel.numberOfLines = 4
        view.addSubview(messageLabel)

        NSLayoutConstraint.activate([
            categoryIcon.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            categoryIcon.topAnchor.constraint(equalTo: view.topAnchor, constant: 16),
            categoryIcon.widthAnchor.constraint(equalToConstant: 32),
            categoryIcon.heightAnchor.constraint(equalToConstant: 32),

            titleLabel.leadingAnchor.constraint(equalTo: categoryIcon.trailingAnchor, constant: 12),
            titleLabel.topAnchor.constraint(equalTo: view.topAnchor, constant: 16),
            titleLabel.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),

            statusLabel.leadingAnchor.constraint(equalTo: categoryIcon.trailingAnchor, constant: 12),
            statusLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 8),
            statusLabel.widthAnchor.constraint(greaterThanOrEqualToConstant: 80),
            statusLabel.heightAnchor.constraint(equalToConstant: 24),

            messageLabel.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            messageLabel.topAnchor.constraint(equalTo: statusLabel.bottomAnchor, constant: 12),
            messageLabel.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),
            messageLabel.bottomAnchor.constraint(lessThanOrEqualTo: view.bottomAnchor, constant: -16),
        ])
    }

    private func configureForCategory(_ category: String) {
        switch category {
        case "IMPL_COMPLETED":
            categoryIcon.image = UIImage(systemName: "checkmark.circle.fill")
            categoryIcon.tintColor = .systemGreen
            statusLabel.text = " Completed "
            statusLabel.backgroundColor = .systemGreen

        case "IMPL_FAILED":
            categoryIcon.image = UIImage(systemName: "xmark.circle.fill")
            categoryIcon.tintColor = .systemRed
            statusLabel.text = " Failed "
            statusLabel.backgroundColor = .systemRed

        case "IMPL_WAITING_INPUT":
            categoryIcon.image = UIImage(systemName: "questionmark.circle.fill")
            categoryIcon.tintColor = .systemOrange
            statusLabel.text = " Waiting for Input "
            statusLabel.backgroundColor = .systemOrange

        case "IMPL_PR_READY":
            categoryIcon.image = UIImage(systemName: "arrow.triangle.pull")
            categoryIcon.tintColor = .systemBlue
            statusLabel.text = " PR Created "
            statusLabel.backgroundColor = .systemBlue

        default:
            categoryIcon.image = UIImage(systemName: "terminal")
            categoryIcon.tintColor = .label
            statusLabel.text = " Update "
            statusLabel.backgroundColor = .systemGray
        }
    }
}
