import "./App.css"
import { useAppDispatch, useAppSelector } from "./app/hooks"
import { useEffect, useState } from "react"
import WebsocketClient from "./features/websocket/WebsocketClient"
import { updateSessionData } from "./features/websocket/websocketDataSlice"
import {
  processFile,
  selectAudioBlobUrl,
  selectFileType,
  selectVideoBlobUrl,
} from "./features/selectedFile/selectedFileSlice"
import { getOrCreateSessionId } from "./utils/getOrCreateSessionId"

const logoUrl =
  "https://media.githubusercontent.com/media/freemocap/skellysubs/3b64fa9bb6843529df050c5373c2773f4bb0e2f4/skellysubs-ui/src/assets/skellysubs-logo.png"

const App = () => {
  const dispatch = useAppDispatch()
  const [selectedFile, setSelectedFile] = useState<File | null>(null)
  // const [ffmpegClient, setFfmpegClient] = useState<FfmpegClient | null>(null)
  const fileType = useAppSelector(selectFileType)
  const audioBlobUrl = useAppSelector(selectAudioBlobUrl)
  const videoBlobUrl = useAppSelector(selectVideoBlobUrl)

  const sessionId = getOrCreateSessionId()
  const webSocketUrl = `ws://localhost:8080/websocket/connect/${sessionId}`

  useEffect(() => {
    // connect to websocket
    const webSocketClient = new WebsocketClient(webSocketUrl)

    // const ffmpegClient = new FfmpegClient()
    // ffmpegClient.load().then(() => setFfmpegClient(ffmpegClient))

    webSocketClient.onMessage(data => {
      dispatch(updateSessionData({ sessionId, data })) // dispatch action to update data in Redux store
    })
    webSocketClient.connect()
    return () => {
      // cleanup
      webSocketClient.close()
    }
  }, [webSocketUrl, sessionId, dispatch])

  const handleFileSelection = (event: React.ChangeEvent<HTMLInputElement>) => {
    const fileSelection: File | undefined = event.target.files?.[0]
    if (fileSelection) {
      console.log(`Selected file: ${fileSelection.name}`)
      setSelectedFile(fileSelection)
    }
  }
  const handleButtonClick = () => {
    if (!selectedFile) {
      console.error("No file selected")
      return
    }
    console.log("Transcribe and Translate button clicked, processing file...")
    dispatch(processFile({ selectedFile }))
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
          onChange={handleFileSelection}
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
        {audioBlobUrl && (
          <audio controls>
            <source src={audioBlobUrl} type="audio/mp3" />
            Your browser does not support the audio element.
          </audio>
        )}
        {videoBlobUrl && (
          <video controls>
            <source src={videoBlobUrl} type="video/mp4" />
            Your browser does not support the video element.
          </video>
        )}
      </header>
    </div>
  )
}

export default App
