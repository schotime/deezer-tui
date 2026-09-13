import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import Quickshell.Services.Mpris
import qs.Commons
import qs.Ui

// This is deliberately an Omarchy-native control surface, not a recreation of
// Deezer's web styling. All colour, font and spacing values come from Color and
// Style, which the shell updates whenever the user changes theme.
BarWidget {
  id: root
  moduleName: "schotime.deezer-player"

  readonly property string binary: String(setting("command", "deezer-tui"))
  readonly property int refreshInterval: Math.max(1000, Number(setting("refreshIntervalMs", 2000)))
  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property color accent: Color.accent
  readonly property color surface: Color.popups.background
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var mprisPlayers: Mpris.players ? Mpris.players.values : []
  readonly property var mprisPlayer: {
    for (var i = 0; i < mprisPlayers.length; i++) {
      var player = mprisPlayers[i]
      var name = String(player.identity || player.desktopEntry || "").toLowerCase()
      if (name.indexOf("deezer") !== -1) return player
    }
    return null
  }
  readonly property string artworkUrl: mprisPlayer ? String(mprisPlayer.trackArtUrl || "") : ""

  property string playbackStatus: "offline"
  property string title: "Deezer is not running"
  property string artist: "Launch deezer-tui to start listening"
  property string album: ""
  property real position: 0
  property real duration: 0
  property int volume: 0
  property string lastError: ""
  property bool opened: false

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function refresh() {
    if (!statusProcess.running) statusProcess.running = true
  }

  function open() { opened = true }
  function close() { opened = false }
  function toggle() { opened = !opened }

  function runAction(action) {
    if (actionProcess.running) return
    actionProcess.action = action
    actionProcess.running = true
  }

  function openTui() {
    // Keep one TUI window: focus the existing app-id window instead of
    // spawning a second terminal instance.
    if (bar) bar.run("hyprctl clients -j | jq -e '.[] | select(.class == \"org.omarchy.deezer-tui\")' >/dev/null "
      + "&& hyprctl dispatch 'hl.dsp.focus({ window = \"class:^(org.omarchy.deezer-tui)$\" })' "
      + "|| omarchy-launch-tui --app-id=org.omarchy.deezer-tui " + root.binary)
    root.close()
  }

  function formatTime(value) {
    var seconds = Math.max(0, Math.floor(Number(value) || 0))
    return Math.floor(seconds / 60) + ":" + ("0" + (seconds % 60)).slice(-2)
  }

  function applyStatus(text) {
    try {
      var data = JSON.parse(String(text))
      playbackStatus = String(data.status || "offline")
      var track = data.track || null
      title = track ? String(track.title || "Unknown track") : "Nothing playing"
      artist = track ? String(track.artist || "") : "Start deezer-tui, then choose music"
      album = track ? String(track.album || "") : ""
      position = Number(data.position_secs || 0)
      duration = Number(data.duration_secs || (track ? track.duration : 0) || 0)
      volume = Number(data.volume_percent || 0)
      lastError = String(data.error || "")
    } catch (error) {
      playbackStatus = "offline"
      title = "Deezer is not running"
      artist = "Launch deezer-tui to start listening"
      album = ""
      position = 0
      duration = 0
      lastError = ""
    }
  }

  onOpenedChanged: if (opened) {
    root.refresh()
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  Timer {
    interval: root.refreshInterval
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  Process {
    id: statusProcess
    command: [root.binary, "--status", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyStatus(text)
    }
  }

  Process {
    id: actionProcess
    property string action: ""
    command: [root.binary, action]
    onExited: root.refresh()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.playbackStatus === "playing" ? "󰎈" : "󰝚"
    active: root.playbackStatus === "playing"
    tooltipText: root.title + (root.artist ? " — " + root.artist : "")
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) root.openTui()
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(360))
    contentHeight: panel.fittedContentHeight(content.implicitHeight, Style.space(310))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onActivateRequested: root.runAction("--toggle")
      onTextKey: function(key) {
        if (key === "n") root.runAction("--next")
        else if (key === "b") root.runAction("--prev")
        else if (key === "r") root.refresh()
        else if (key === "o") root.openTui()
      }

      Column {
        id: content
        width: parent.width
        spacing: Style.space(12)

        Row {
          width: parent.width
          spacing: Style.space(12)

          Rectangle {
            width: Style.space(52)
            height: width
            radius: Style.cornerRadius
            color: Qt.rgba(root.accent.r, root.accent.g, root.accent.b, 0.18)
            clip: true

            Image {
              anchors.fill: parent
              visible: root.artworkUrl !== ""
              source: root.artworkUrl
              fillMode: Image.PreserveAspectCrop
              asynchronous: true
            }

            Text {
              anchors.centerIn: parent
              visible: root.artworkUrl === ""
              text: root.playbackStatus === "playing" ? "󰎈" : "󰝚"
              color: root.accent
              font.family: root.fontFamily
              font.pixelSize: Style.font.display
            }
          }

          Column {
            width: parent.width - Style.space(64)
            spacing: Style.space(3)

            Text {
              width: parent.width
              text: root.title
              textFormat: Text.PlainText
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              font.bold: true
              elide: Text.ElideRight
            }
            Text {
              width: parent.width
              text: root.artist
              textFormat: Text.PlainText
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              elide: Text.ElideRight
            }
            Text {
              visible: root.album !== ""
              width: parent.width
              text: root.album
              textFormat: Text.PlainText
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }
          }
        }

        Rectangle {
          width: parent.width
          height: Style.space(4)
          radius: height / 2
          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.15)
          visible: root.duration > 0

          Rectangle {
            width: parent.width * Math.min(1, root.position / root.duration)
            height: parent.height
            radius: parent.radius
            color: root.accent
          }
        }

        Row {
          width: parent.width
          visible: root.duration > 0
          Text { id: currentTime; text: root.formatTime(root.position); color: root.dim; font.family: root.fontFamily; font.pixelSize: Style.font.caption }
          Item { width: parent.width - currentTime.implicitWidth - rightTime.implicitWidth; height: 1 }
          Text { id: rightTime; text: root.formatTime(root.duration); color: root.dim; font.family: root.fontFamily; font.pixelSize: Style.font.caption }
        }

        Row {
          width: parent.width
          spacing: Style.space(8)

          Button { text: "󰒮"; enabled: root.playbackStatus !== "offline"; onClicked: root.runAction("--prev") }
          Button { text: root.playbackStatus === "playing" ? "󰏤" : "󰐊"; enabled: root.playbackStatus !== "offline"; onClicked: root.runAction("--toggle") }
          Button { text: "󰒭"; enabled: root.playbackStatus !== "offline"; onClicked: root.runAction("--next") }
          Item { width: 1; height: 1 }
          Button { text: "Open TUI"; onClicked: root.openTui() }
        }

        Text {
          visible: root.playbackStatus === "offline"
          width: parent.width
          text: "Right-click the bar icon or use Open TUI to launch deezer-tui."
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }
      }
    }
  }
}
