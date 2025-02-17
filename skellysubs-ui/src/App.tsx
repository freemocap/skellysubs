import "./App.css"
import logo from "./assets/skellysubs-logo.png"

const App = () => {
  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (file) {
      console.log(`Selected file: ${file.name}`)
    }
  }

  return (
    <div className="App">
      <header className="App-header">
        <img src={logo} className="App-logo" alt="logo" />
        <h1>Welcome to SkellySubs!</h1>
        <p>Upload a video or audio file to get started.</p>
        <input
          type="file"
          accept="audio/*,video/*"
          onChange={handleFileUpload}
          style={{ display: "block", margin: "20px 0" }}
        />

        <button type="submit" className="App-button">
          Transcribe and Translate
        </button>
        <p className="App-hint">
          (Hint: Open the browser tools with F12 (Windows) or Cmd+Option+I
          (macOS) and check the console for progress)
        </p>
      </header>
    </div>
  )
}

export default App
