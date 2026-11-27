import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Value editor for one factor: level list for categorical factors, text otherwise.
// `value` stays bound to the owner's data: edits only emit `edited`, and the
// owner stores the new value.
Item {
    id: root
    property var factor: null
    property string value: ""
    signal edited(string value)
    implicitHeight: loader.implicitHeight
    implicitWidth: 140

    Loader {
        id: loader
        anchors.left: parent.left
        anchors.right: parent.right
        sourceComponent: root.factor && root.factor.categorical ? levels : field
    }
    Component {
        id: levels
        FitComboBox {
            model: root.factor.levels
            currentIndex: root.factor.levels.indexOf(root.value)
            // A value that is not one of the levels shows as it is.
            displayText: currentIndex < 0 ? root.value : currentText
            onActivated: root.edited(currentText)
        }
    }
    Component {
        id: field
        TextField {
            readonly property bool numeric: text.trim() === "" || isFinite(Number(text.trim().replace(",", ".")))
            text: root.value
            horizontalAlignment: Text.AlignRight
            selectByMouse: true
            color: numeric ? Theme.text : Theme.error
            onEditingFinished: if (text !== root.value) root.edited(text)
        }
    }
}
