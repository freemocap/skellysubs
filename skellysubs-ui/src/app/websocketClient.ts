// src/app/websocket.ts
class WebsocketClient {
  socket: WebSocket | null = null
  private url: string
  private onMessageCallback: ((data: any) => void) | null = null

  constructor(url: string) {
    this.url = url
  }

  connect() {
    this.socket = new WebSocket(this.url)
    this.socket.onopen = () => console.log("WebSocket connected")
    this.socket.onclose = () => this.reconnect()
    this.socket.onmessage = event => {
      const data = JSON.parse(event.data)
      if (this.onMessageCallback) {
        this.onMessageCallback(data)
      }
    }
  }

  reconnect() {
    console.log("WebSocket disconnected, attempting to reconnect...")
    setTimeout(() => this.connect(), 5000)
  }

  sendMessage(message: Object) {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message))
    }
  }

  onMessage(callback: (data: any) => void) {
    this.onMessageCallback = callback
  }

  close() {
    this.socket?.close()
  }
}

export default WebsocketClient
