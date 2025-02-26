import type { ReactNode } from "react"
import { useEffect } from "react"
import { useState } from "react"
import { createContext, useContext } from "react"
import { ffmpegService } from "./useFfmpeg"
import {useAppDispatch} from "../../store/hooks";
interface FfmpegContextProps {
  isLoaded: boolean
  convertToMP3: typeof ffmpegService.convertToMP3
  error: string | null
}

interface FfmpegProviderProps {
  children: ReactNode
}

const FfmpegContext = createContext<FfmpegContextProps | undefined>(undefined)

export const FfmpegContextProvider = ({ children }) => {
  const [isLoaded, setIsLoaded] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const dispatch = useAppDispatch()

  useEffect(() => {
    const loadFfmpeg = async () => {
      try {
        await ffmpegService.loadFfmpeg(dispatch)
        setIsLoaded(true)
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load FFmpeg")
      }
    }

    if (!ffmpegService.isLoaded) loadFfmpeg()
  }, [dispatch])

  return (
      <FfmpegContext.Provider
          value={{
            isLoaded,
            convertToMP3: ffmpegService.convertToMP3.bind(ffmpegService),
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
