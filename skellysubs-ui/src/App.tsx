import "./App.css"
import { useAppDispatch } from "./app/hooks"
import { useEffect } from "react"
import WebsocketClient from "./app/websocketClient"
import { updateSessionData } from "./features/websocket/websocketSlice"
import { v4 as uuidv4 } from "uuid"
import { WebsocketDataDisplay } from "./features/websocket/WebsocketDataDisplay"

const logoUrl =
  "https://media.githubusercontent.com/media/freemocap/skellysubs/3b64fa9bb6843529df050c5373c2773f4bb0e2f4/skellysubs-ui/src/assets/skellysubs-logo.png"

const App = () => {
  const dispatch = useAppDispatch()
  // create uuid for session id
  const sessionId = uuidv4()
  const webSocketUrl = `ws://localhost:8080/websocket/connect/${sessionId}`
  useEffect(() => {
    // connect to websocket
    const webSocketClient = new WebsocketClient(webSocketUrl)

    webSocketClient.onMessage(data => {
      dispatch(updateSessionData({ sessionId, data })) // dispatch action to update data in Redux store
    })
    webSocketClient.connect()
    return () => {
      // cleanup
      webSocketClient.close()
    }
  }, [webSocketUrl, sessionId, dispatch])

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (file) {
      console.log(`Selected file: ${file.name}`)
    }
  }

  const handleButtonClick = () => {
    console.log("Transcribe and Translate button clicked, wow")
  }

  return (
    <div className="App">
      <header className="App-header">
        <div
          className="App-logo"
          style={{ backgroundImage: `url(${logoUrl})` }}
        ></div>
        <h1>Welcome to SkellySubs!</h1>
        <p>Upload a video or audio file to get started.</p>
        <input
          type="file"
          accept="audio/*,video/*"
          onChange={handleFileUpload}
          style={{ display: "block", margin: "20px 0" }}
        />

        <button
          onClick={handleButtonClick}
          type="submit"
          className="App-button"
        >
          Transcribe and Translate
        </button>
        <p className="App-hint">
          (Hint: Open the browser tools with F12 (Windows) or Cmd+Option+I
          (macOS) and check the console for progress)
        </p>
        <WebsocketDataDisplay />
      </header>
    </div>
  )
}

export default App
