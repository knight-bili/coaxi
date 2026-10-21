import UIKit
import SwiftTerm

/// A clean input-accessory key bar for the terminal, replacing SwiftTerm's stock
/// ``TerminalAccessory``.
///
/// The stock bar shipped two buttons that make no sense for a remote-shell client
/// and confused users — both sat in the bottom-right where a *dismiss* button is
/// expected:
///   * `hand.draw` toggled `allowMouseReporting` with no visible effect ("tapped,
///     nothing happens"), and
///   * `keyboard.chevron.compact.down` swapped the system keyboard for SwiftTerm's
///     own on-screen keyboard grid ("tapped, wrong thing happens").
/// It also rendered chunky shadowed gray keys that read as a stray "bit of
/// keyboard" rather than a polished bar.
///
/// This bar drops those and offers only what a phone keyboard lacks: the
/// essentials (Esc / Ctrl / Tab) on the leading edge, then the arrow cluster and a
/// working keyboard-**dismiss** button on the trailing edge — so the bottom-right
/// key does the obvious thing. The row lives in a horizontal scroll view purely as
/// an anti-clip fallback (it only scrolls on a device too narrow to show every
/// key); on normal widths a flexible spacer holds the two groups apart.
///
/// Ctrl is a sticky modifier: tapping it arms `TerminalView.controlModifier` — the
/// public flag SwiftTerm falls back to when the accessory isn't its own
/// `TerminalAccessory` (it reads `terminalAccessory?.controlModifier ?? controlModifier`).
/// The terminal consumes and clears it on the next key; we observe
/// `.terminalViewControlModifierReset` to clear the button's armed look in sync.
final class TerminalKeyBar: UIInputView {
    private weak var terminalView: SwiftTerm.TerminalView?
    private var ctrlKey: UIButton?
    private var ctrlArmed = false { didSet { refreshCtrl() } }

    private let barHeight: CGFloat = 44

    init(terminalView: SwiftTerm.TerminalView) {
        self.terminalView = terminalView
        super.init(frame: CGRect(x: 0, y: 0, width: 0, height: 44), inputViewStyle: .keyboard)
        allowsSelfSizing = true
        translatesAutoresizingMaskIntoConstraints = false
        build()
        // Keep the armed look in sync when the terminal consumes (and resets) the
        // control modifier on the next keystroke. Scoped to this terminal view.
        NotificationCenter.default.addObserver(
            self, selector: #selector(controlDidReset),
            name: .terminalViewControlModifierReset, object: terminalView)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    deinit { NotificationCenter.default.removeObserver(self) }

    // The system sizes an input accessory from its intrinsic height.
    override var intrinsicContentSize: CGSize {
        CGSize(width: UIView.noIntrinsicMetric, height: barHeight)
    }

    // MARK: - Layout

    private func build() {
        // The keys a phone keyboard lacks, and nothing more: Esc / Ctrl / Tab on the
        // left, arrows then a keyboard-dismiss on the right. (Shell symbols like | ~
        // live on the system keyboard's symbol plane, so they're omitted to keep
        // every key visible without scrolling.) The flexible spacer pushes the arrow
        // cluster + dismiss to the trailing edge when everything fits; on a very
        // narrow device the row scrolls horizontally rather than clipping a key.
        let keys = UIStackView(arrangedSubviews: [
            key(title: "esc", action: #selector(esc)),
            ctrlButton(),
            key(symbol: "arrow.right.to.line", action: #selector(tab)),
            flexibleSpacer(),
            key(symbol: "arrow.left", action: #selector(arrowLeft)),
            key(symbol: "arrow.down", action: #selector(arrowDown)),
            key(symbol: "arrow.up", action: #selector(arrowUp)),
            key(symbol: "arrow.right", action: #selector(arrowRight)),
            dismissKey(),
        ])
        keys.axis = .horizontal
        keys.spacing = 5
        keys.alignment = .center
        keys.translatesAutoresizingMaskIntoConstraints = false

        let scroll = UIScrollView()
        scroll.showsHorizontalScrollIndicator = false
        scroll.alwaysBounceHorizontal = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(keys)
        addSubview(scroll)

        let margins = layoutMarginsGuide
        let content = scroll.contentLayoutGuide
        let frameG = scroll.frameLayoutGuide
        // Make the stack at least as wide as the visible area so the spacer can push
        // the trailing cluster to the edge; a wider stack (overflow) just scrolls.
        let stackWidth = keys.widthAnchor.constraint(greaterThanOrEqualTo: frameG.widthAnchor)
        stackWidth.priority = .required

        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: margins.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: margins.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: topAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),

            keys.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            keys.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            keys.topAnchor.constraint(equalTo: content.topAnchor),
            keys.bottomAnchor.constraint(equalTo: content.bottomAnchor),
            keys.heightAnchor.constraint(equalTo: frameG.heightAnchor),
            stackWidth,
        ])
    }

    private func dismissKey() -> UIButton {
        let button = key(symbol: "keyboard.chevron.compact.down", action: #selector(hideKeyboard))
        button.accessibilityLabel = "Hide Keyboard"
        return button
    }

    // MARK: - Buttons

    private func key(title: String? = nil, symbol: String? = nil, action: Selector) -> UIButton {
        var config = UIButton.Configuration.gray()
        config.cornerStyle = .medium
        config.baseForegroundColor = .label
        config.baseBackgroundColor = .secondarySystemFill
        config.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 8, bottom: 6, trailing: 8)
        if let title {
            var attr = AttributeContainer()
            attr.font = UIFont.monospacedSystemFont(ofSize: 15, weight: .medium)
            config.attributedTitle = AttributedString(title, attributes: attr)
        }
        if let symbol {
            config.image = UIImage(
                systemName: symbol,
                withConfiguration: UIImage.SymbolConfiguration(pointSize: 15, weight: .medium))
        }
        let button = UIButton(configuration: config)
        button.addTarget(self, action: action, for: .touchUpInside)
        button.setContentHuggingPriority(.required, for: .horizontal)
        button.setContentCompressionResistancePriority(.required, for: .horizontal)
        return button
    }

    private func ctrlButton() -> UIButton {
        let button = key(title: "ctrl", action: #selector(toggleCtrl))
        ctrlKey = button
        return button
    }

    private func flexibleSpacer() -> UIView {
        let view = UIView()
        view.setContentHuggingPriority(.defaultLow, for: .horizontal)
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        view.widthAnchor.constraint(greaterThanOrEqualToConstant: 8).isActive = true
        return view
    }

    /// Invert the Ctrl key while armed so its sticky state is obvious. Accent-
    /// agnostic (label-on-background) to avoid bridging the SwiftUI theme accent
    /// into UIKit here.
    private func refreshCtrl() {
        guard let button = ctrlKey, var config = button.configuration else { return }
        config.baseBackgroundColor = ctrlArmed ? .label : .secondarySystemFill
        config.baseForegroundColor = ctrlArmed ? .systemBackground : .label
        button.configuration = config
    }

    // MARK: - Actions

    @objc private func esc() { send([0x1b]) }
    @objc private func tab() { send([0x09]) }

    @objc private func arrowUp() { send(arrow(.up)) }
    @objc private func arrowDown() { send(arrow(.down)) }
    @objc private func arrowLeft() { send(arrow(.left)) }
    @objc private func arrowRight() { send(arrow(.right)) }

    @objc private func hideKeyboard() { terminalView?.resignFirstResponder() }

    @objc private func toggleCtrl() {
        guard let view = terminalView else { return }
        ctrlArmed.toggle()
        view.controlModifier = ctrlArmed
    }

    @objc private func controlDidReset() { ctrlArmed = false }

    private func send(_ bytes: [UInt8]) {
        UIDevice.current.playInputClick()
        terminalView?.send(bytes)
    }

    private enum Arrow { case up, down, left, right }

    /// Arrows depend on the cursor-key mode (DECCKM): apps like vim/less switch the
    /// terminal to application-cursor mode, where arrows are `ESC O A` rather than
    /// `ESC [ A`. Mirror SwiftTerm's own `sendKeyUp`/etc. so history recall and
    /// editor navigation both work.
    private func arrow(_ direction: Arrow) -> [UInt8] {
        let app = terminalView?.getTerminal().applicationCursor ?? false
        switch direction {
        case .up: return app ? EscapeSequences.moveUpApp : EscapeSequences.moveUpNormal
        case .down: return app ? EscapeSequences.moveDownApp : EscapeSequences.moveDownNormal
        case .left: return app ? EscapeSequences.moveLeftApp : EscapeSequences.moveLeftNormal
        case .right: return app ? EscapeSequences.moveRightApp : EscapeSequences.moveRightNormal
        }
    }
}

// Lets the keys play the system keyboard click when input clicks are enabled.
extension TerminalKeyBar: UIInputViewAudioFeedback {
    var enableInputClicksWhenVisible: Bool { true }
}
