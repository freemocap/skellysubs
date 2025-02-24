import type { ReactNode } from "react"
import { useEffect } from "react"
import { useState } from "react"
import { createContext, useContext } from "react"
import { ffmpegService } from "./useFfmpeg"
interface FfmpegContextProps {
  isLoaded: boolean
  extractAudioFromVideo: (file: File) => Promise<void>
  error: string | null
}

interface FfmpegProviderProps {
  children: ReactNode
}

const FfmpegContext = createContext<FfmpegContextProps | undefined>(undefined)

export const FfmpegContextProvider: React.FC<FfmpegProviderProps> = ({
  children,
}) => {
  const [isLoaded, setIsLoaded] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const loadFfmpeg = async () => {
      try {
        await ffmpegService.loadFfmpeg()
        setIsLoaded(true)
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load FFmpeg")
      }
    }

    if (!ffmpegService.isLoaded) loadFfmpeg()
  }, [])

  return (
    <FfmpegContext.Provider
      value={{
        isLoaded,
        extractAudioFromVideo:
          ffmpegService.extractAudioFromVideo.bind(ffmpegService),
        error,
      }}
    >
      {children}
    </FfmpegContext.Provider>
  )
}

export const useFfmpegContext = () => {
  const context = useContext(FfmpegContext)
  if (!context) {
    throw new Error("useFfmpegContext must be used within a FfmpegProvider")
  }
  return context
}
