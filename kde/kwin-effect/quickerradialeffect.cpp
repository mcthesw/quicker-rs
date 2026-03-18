#include <effect/effect.h>
#include <effect/effecthandler.h>
#include <effect/effectwindow.h>

#include <QAction>
#include <QDBusConnection>
#include <QDBusConnectionInterface>
#include <QDBusMessage>
#include <QDBusServiceWatcher>
#include <QMouseEvent>
#include <QString>
#include <QStringList>

namespace
{
constexpr auto kBridgeBusName = "net.quicker_rs.KWinBridge";
constexpr auto kBridgePath = "/net/quicker_rs/KWinBridge";
constexpr auto kBridgeInterface = "net.quicker_rs.KWinBridge";

bool isBrowserWindow(const KWin::EffectWindow *window)
{
    if (!window) {
        return false;
    }

    const QString windowClass = window->windowClass().toLower();
    static const QStringList browserPatterns = {
        QStringLiteral("chrome"),
        QStringLiteral("chromium"),
        QStringLiteral("firefox"),
        QStringLiteral("msedge"),
        QStringLiteral("edge"),
        QStringLiteral("brave"),
        QStringLiteral("opera"),
        QStringLiteral("vivaldi"),
        QStringLiteral("zen"),
        QStringLiteral("safari"),
    };

    for (const QString &pattern : browserPatterns) {
        if (windowClass.contains(pattern)) {
            return true;
        }
    }

    return false;
}

QString buttonName(Qt::MouseButton button)
{
    if (button == Qt::RightButton) {
        return QStringLiteral("right");
    }
    return QStringLiteral("middle");
}
}

class QuickerRadialEffect final : public KWin::Effect
{
    Q_OBJECT

public:
    QuickerRadialEffect()
        : m_rightAction(new QAction(this))
        , m_middleAction(new QAction(this))
        , m_serviceWatcher(new QDBusServiceWatcher(
              QString::fromLatin1(kBridgeBusName),
              QDBusConnection::sessionBus(),
              QDBusServiceWatcher::WatchForRegistration | QDBusServiceWatcher::WatchForUnregistration,
              this))
    {
        connect(m_rightAction, &QAction::triggered, this, [this] {
            startGesture(Qt::RightButton);
        });
        connect(m_middleAction, &QAction::triggered, this, [this] {
            startGesture(Qt::MiddleButton);
        });

        KWin::effects->registerPointerShortcut(Qt::NoModifier, Qt::RightButton, m_rightAction);
        KWin::effects->registerPointerShortcut(Qt::NoModifier, Qt::MiddleButton, m_middleAction);

        connect(KWin::effects, &KWin::EffectsHandler::windowActivated, this, [this] {
            updateShortcutState();
        });
        connect(m_serviceWatcher, &QDBusServiceWatcher::serviceRegistered, this, [this] {
            updateShortcutState();
        });
        connect(m_serviceWatcher, &QDBusServiceWatcher::serviceUnregistered, this, [this] {
            if (m_gestureActive) {
                finishGesture(KWin::effects->cursorPos());
            }
            updateShortcutState();
        });

        updateShortcutState();
    }

    ~QuickerRadialEffect() override
    {
        if (m_gestureActive) {
            KWin::effects->stopMouseInterception(this);
        }
    }

    bool isActive() const override
    {
        return m_gestureActive;
    }

    void windowInputMouseEvent(QEvent *event) override
    {
        if (!m_gestureActive) {
            return;
        }

        auto *mouseEvent = dynamic_cast<QMouseEvent *>(event);
        if (!mouseEvent) {
            return;
        }

        const QPointF globalPos = mouseEvent->globalPosition();
        switch (event->type()) {
        case QEvent::MouseMove:
            sendSignal(QStringLiteral("GestureMove"), {globalPos.x(), globalPos.y()});
            event->accept();
            break;
        case QEvent::MouseButtonRelease:
            if (mouseEvent->button() == m_activeButton) {
                finishGesture(globalPos);
                event->accept();
            }
            break;
        default:
            break;
        }
    }

private:
    void startGesture(Qt::MouseButton button)
    {
        if (m_gestureActive || !isBridgeAvailable()) {
            return;
        }

        m_activeButton = button;
        m_gestureActive = true;
        KWin::effects->startMouseInterception(this, Qt::ArrowCursor);

        const QPointF pos = KWin::effects->cursorPos();
        sendSignal(QStringLiteral("GestureStart"),
                   {pos.x(), pos.y(), buttonName(button), activeWindowClass()});
    }

    void finishGesture(const QPointF &pos)
    {
        sendSignal(QStringLiteral("GestureEnd"), {pos.x(), pos.y()});
        KWin::effects->stopMouseInterception(this);
        m_gestureActive = false;
        m_activeButton = Qt::NoButton;
    }

    void updateShortcutState()
    {
        const bool bridgeAvailable = isBridgeAvailable();
        const bool browser = isBrowserWindow(KWin::effects->activeWindow());

        m_rightAction->setEnabled(bridgeAvailable && browser);
        m_middleAction->setEnabled(bridgeAvailable && !browser);
    }

    bool isBridgeAvailable() const
    {
        auto *bus = QDBusConnection::sessionBus().interface();
        return bus && bus->isServiceRegistered(QString::fromLatin1(kBridgeBusName));
    }

    QString activeWindowClass() const
    {
        if (const KWin::EffectWindow *window = KWin::effects->activeWindow()) {
            return window->windowClass();
        }
        return QString();
    }

    void sendSignal(const QString &member, const QVariantList &arguments)
    {
        QDBusMessage message = QDBusMessage::createSignal(
            QString::fromLatin1(kBridgePath),
            QString::fromLatin1(kBridgeInterface),
            member);
        message.setArguments(arguments);
        QDBusConnection::sessionBus().send(message);
    }

    QAction *const m_rightAction;
    QAction *const m_middleAction;
    QDBusServiceWatcher *const m_serviceWatcher;
    Qt::MouseButton m_activeButton = Qt::NoButton;
    bool m_gestureActive = false;
};

KWIN_EFFECT_FACTORY(QuickerRadialEffect, "quickerradialeffect.json")

#include "quickerradialeffect.moc"
