#include <QtQuickTest>
#include <QQmlContext>
#include <QQmlEngine>
#include <cstdint>

extern "C" {
void vdisc_audio_test_enable();
void vdisc_audio_test_set_position(std::uint64_t);
void vdisc_audio_test_queue_event(std::uint32_t);
void vdisc_audio_test_fail_next_open(std::uint32_t);
float vdisc_audio_test_gain();
}

class AudioTestDriver : public QObject {
    Q_OBJECT
public:
    using QObject::QObject;
    Q_INVOKABLE void setPosition(quint64 position) { vdisc_audio_test_set_position(position); }
    Q_INVOKABLE void queueEvent(uint event) { vdisc_audio_test_queue_event(event); }
    Q_INVOKABLE void failNextOpen(uint code) { vdisc_audio_test_fail_next_open(code); }
    Q_INVOKABLE float gain() const { return vdisc_audio_test_gain(); }
};

class TestSetup : public QObject {
    Q_OBJECT
public slots:
    void applicationAvailable() { vdisc_audio_test_enable(); }
    void qmlEngineAvailable(QQmlEngine *engine) {
        engine->rootContext()->setContextProperty("audioTestDriver", new AudioTestDriver(engine));
    }
};

QUICK_TEST_MAIN_WITH_SETUP(vdisc_appliance, TestSetup)
#include "quick_test_main.moc"
