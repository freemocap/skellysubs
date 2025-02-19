import "./App.css"
import { useAppDispatch, useAppSelector } from "./app/hooks"
import { useEffect, useState } from "react"

import AudioExtractor from "./features/selectedFile/AudioExtractor"

const logoUrl =
  "https://media.githubusercontent.com/media/freemocap/skellysubs/3b64fa9bb6843529df050c5373c2773f4bb0e2f4/skellysubs-ui/src/assets/skellysubs-logo.png"

const App = () => {
  const dispatch = useAppDispatch()
  const [selectedFile, setSelectedFile] = useState<File | null>(null)

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
    console.log("Transcribe and Translate button clicked,wowZa!")
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
        [NOTE - Doesn't do anything yet, functionality coming soon!].
        <AudioExtractor />
        {/*<input*/}
        {/*  type="file"*/}
        {/*  accept="audio/*"*/}
        {/*  onChange={handleFileSelection}*/}
        {/*  style={{ display: "block", margin: "20px 0" }}*/}
        {/*/>*/}
        {/*<button*/}
        {/*  onClick={handleButtonClick}*/}
        {/*  disabled={!selectedFile}*/}
        {/*  type="submit"*/}
        {/*  className="App-button"*/}
        {/*>*/}
        {/*  Transcribe and Translate (Coming Soon!)*/}
        {/*</button>*/}
        <p className="App-hint">
          (Hint: Open the browser tools with F12 (Windows) or Cmd+Option+I
          (macOS) and check the console for progress)
        </p>
      </header>
    </div>
  )
}

export default App
