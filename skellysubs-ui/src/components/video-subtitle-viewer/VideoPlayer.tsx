import { useCallback, useEffect, useRef, useState } from "react"
import { IconButton, Stack, Typography } from "@mui/material"
import { Pause, PlayArrow, VolumeOff, VolumeUp } from "@mui/icons-material"
import type { Subtitle, SubtitleCue } from "./video-subtitle-viewer-types"
import {
  StyledSlider,
  SubtitleOverlay,
  VideoContainer,
  VideoControls,
  VideoWrapper,
} from "./video-subtitle-viewer-styles"
import { formatTime } from "./video-subtitle-viewer-utils"

interface VideoPlayerProps {
  mediaFile?: { url: string; type: string }
  currentSubtitle?: Subtitle | null
  selectedSubtitle?: SubtitleCue | null
  onTimeUpdate: (time: number) => void
  currentTime?: number
}

export const VideoPlayer = ({
  mediaFile,
  currentSubtitle,
  selectedSubtitle,
  onTimeUpdate,
  currentTime,
}: VideoPlayerProps) => {
  const videoRef = useRef<HTMLVideoElement>(null)
  const [isPlaying, setIsPlaying] = useState(false)
  const [isMuted, setIsMuted] = useState(false)
  const [duration, setDuration] = useState(0)
  const lastUpdate = useRef(Date.now())
  const isSeeking = useRef(false)
  const [localTime, setLocalTime] = useState(0)
  const [videoDimensions, setVideoDimensions] = useState({
    width: 0,
    height: 0,
  })
  const containerRef = useRef<HTMLDivElement>(null)
  const throttledTimeUpdate = useCallback(
    (time: number) => {
      if (Date.now() - lastUpdate.current > 100) {
        setLocalTime(time)
        onTimeUpdate(time)
        lastUpdate.current = Date.now()
      }
    },
    [onTimeUpdate],
  )
  useEffect(() => {
    const video = videoRef.current
    const container = containerRef.current
    if (!video || !container) return

    const calculateDimensions = () => {
      const containerWidth = container.clientWidth
      const containerHeight = container.clientHeight
      const videoRatio = video.videoWidth / video.videoHeight
      const containerRatio = containerWidth / containerHeight

      let width, height

      // If video is wider than container (relative to their ratios)
      if (videoRatio > containerRatio) {
        width = containerWidth
        height = containerWidth / videoRatio
      } else {
        height = containerHeight
        width = containerHeight * videoRatio
      }

      setVideoDimensions({ width, height })
    }

    const handleResize = () => {
      calculateDimensions()
    }

    // Calculate on video metadata load
    const handleLoadedMetadata = () => {
      calculateDimensions()
    }

    video.addEventListener("loadedmetadata", handleLoadedMetadata)
    window.addEventListener("resize", handleResize)

    // Initial calculation
    if (video.videoWidth) {
      calculateDimensions()
    }

    return () => {
      video.removeEventListener("loadedmetadata", handleLoadedMetadata)
      window.addEventListener("resize", handleResize)
    }
  }, [])

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const handleLoadedData = () => setDuration(video.duration)
    const handleTimeUpdate = () =>
      !isSeeking.current && throttledTimeUpdate(video.currentTime)

    video.addEventListener("loadeddata", handleLoadedData)
    video.addEventListener("timeupdate", handleTimeUpdate)

    return () => {
      video.removeEventListener("loadeddata", handleLoadedData)
      video.removeEventListener("timeupdate", handleTimeUpdate)
    }
  }, [throttledTimeUpdate])

  useEffect(() => {
    if (!videoRef.current || currentTime === undefined || isSeeking.current)
      return
    const diff = Math.abs(videoRef.current.currentTime - currentTime)
    if (diff > 0.1) videoRef.current.currentTime = currentTime
  }, [currentTime])

  const handlePlayPause = () => {
    if (videoRef.current) {
      isPlaying ? videoRef.current.pause() : videoRef.current.play()
      setIsPlaying(!isPlaying)
    }
  }

  const handleMuteToggle = () => {
    if (videoRef.current) {
      videoRef.current.muted = !isMuted
      setIsMuted(!isMuted)
    }
  }

  const handleSeekStart = () => {
    isSeeking.current = true
    if (videoRef.current && isPlaying) {
      videoRef.current.pause()
    }
  }

  const handleSeek = (_: Event, value: number | number[]) => {
    const newTime = Array.isArray(value) ? value[0] : value
    if (videoRef.current) {
      videoRef.current.currentTime = newTime
      throttledTimeUpdate(newTime)
    }
  }

  const handleSeekEnd = () => {
    isSeeking.current = false
    if (videoRef.current && isPlaying) {
      videoRef.current.play()
    }
  }

  return (
    <VideoWrapper>
      <VideoContainer ref={containerRef}>
        <video
          ref={videoRef}
          style={{
            width: videoDimensions.width ? `${videoDimensions.width}px` : '100%',
            height: videoDimensions.height ? `${videoDimensions.height}px` : '100%',
            maxWidth: '100%',
            maxHeight: 'calc(100% - 70px)', // Account for controls
            objectFit: 'contain', // This is key - ensures the whole video is visible
            display: 'block', // Prevents unwanted spacing
                      }}
          src={mediaFile?.url}
          onPlay={() => setIsPlaying(true)}
          onPause={() => setIsPlaying(false)}
        >
          {mediaFile?.type?.startsWith("audio") && (
            <Typography
              variant="h6"
              sx={{ color: "white", textAlign: "center" }}
            >
              Audio Visualization
            </Typography>
          )}
          {selectedSubtitle?.content && (
            <track
              kind="subtitles"
              src={URL.createObjectURL(
                new Blob([selectedSubtitle.content], {
                  type: "text/vtt",
                }),
              )}
              srcLang="en"
              label="English"
              default
            />
          )}
        </video>
        <SubtitleOverlay>
          <Typography
            variant="body1"
            sx={{
              color: "rgba(255,255,155)",
              backgroundColor: "rgba(0,0,0,0.75)",
              display: "inline-block",
              px: 2,
              py: 1,
              borderRadius: 1,
            }}
          >
            {currentSubtitle?.text.join(" ") || ""}
          </Typography>
        </SubtitleOverlay>
        <VideoControls>
          <Stack direction="row" alignItems="center" spacing={1} width="100%">
            <IconButton
              onClick={handlePlayPause}
              sx={{ color: "common.white" }}
            >
              {isPlaying ? <Pause /> : <PlayArrow />}
            </IconButton>
            <IconButton
              onClick={handleMuteToggle}
              sx={{ color: "common.white" }}
            >
              {isMuted ? <VolumeOff /> : <VolumeUp />}
            </IconButton>
            <Typography
              variant="body2"
              sx={{ color: "common.white", width: 100 }}
            >
              {formatTime(videoRef.current?.currentTime || 0)}
            </Typography>
            <StyledSlider
              value={localTime}
              min={0}
              max={duration}
              step={0.1}
              onChange={handleSeek}
              onChangeCommitted={handleSeekEnd}
              onMouseDown={handleSeekStart}
              onTouchStart={handleSeekStart}
              sx={{ mx: 2 }}
              aria-labelledby="video-slider"
            />
            <Typography
              variant="body2"
              sx={{ color: "common.white", width: 100 }}
            >
              {formatTime(localTime)}
            </Typography>
          </Stack>
        </VideoControls>
      </VideoContainer>
    </VideoWrapper>
  )
}
