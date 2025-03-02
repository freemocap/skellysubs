import type React from "react"
import { PaperbaseContent } from "./layout/paperbase_theme/PaperbaseContent"
import { Provider } from "react-redux"
import { AppStateStore } from "./store/appStateStore"
import { FfmpegContextProvider } from "./services/FfmpegService/FfmpegContext"
import { initializeLogger } from "./utils/logger"
initializeLogger(AppStateStore)
const App: React.FC = () => {
  return (
    <Provider store={AppStateStore}>
      <FfmpegContextProvider>
        <PaperbaseContent />
      </FfmpegContextProvider>
    </Provider>
  )
}

export default App
