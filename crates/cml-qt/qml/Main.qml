// CleanMyLinux — KDE/Kirigami main window.
import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import io.cleanmylinux

Kirigami.ApplicationWindow {
    id: root
    title: "CleanMyLinux"
    width: 980
    height: 680
    visible: true

    // Shared engine controller (Rust QObject exposed via cxx-qt).
    Controller {
        id: controller
    }

    // Module sidebar (Kirigami global drawer in sidebar mode).
    globalDrawer: Kirigami.GlobalDrawer {
        isMenu: false
        modal: false
        title: "CleanMyLinux"
        titleIcon: "io.cleanmylinux.CleanMyLinux"

        actions: [
            Kirigami.Action {
                text: "Smart Scan"
                icon.name: "edit-find"
                onTriggered: { controller.scan(0); pageStack.replace(smartPage); }
            },
            Kirigami.Action {
                text: "System Junk"
                icon.name: "user-trash"
                onTriggered: { controller.scan(1); pageStack.replace(smartPage); }
            },
            Kirigami.Action {
                text: "Package Cleanup"
                icon.name: "package-x-generic"
                onTriggered: { controller.scan(2); pageStack.replace(smartPage); }
            },
            Kirigami.Action {
                text: "Large & Old Files"
                icon.name: "folder-documents"
                onTriggered: { controller.scan(3); pageStack.replace(smartPage); }
            },
            Kirigami.Action {
                text: "System Monitor"
                icon.name: "utilities-system-monitor"
                onTriggered: pageStack.replace(monitorPage)
            }
        ]
    }

    pageStack.initialPage: smartPage

    Component {
        id: smartPage
        SmartScanPage { controllerRef: controller }
    }

    Component {
        id: monitorPage
        MonitorPage { controllerRef: controller }
    }
}
