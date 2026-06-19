// Live system monitor — polls the Rust controller once per second.
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Kirigami.ScrollablePage {
    id: page
    title: "System Monitor"

    property var controllerRef
    property string sample: ""

    Timer {
        interval: 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: if (controllerRef) page.sample = controllerRef.sample_monitor()
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Heading {
            text: "System Monitor"
            level: 2
        }
        Controls.Label {
            text: page.sample
            font.family: "monospace"
        }
    }
}
