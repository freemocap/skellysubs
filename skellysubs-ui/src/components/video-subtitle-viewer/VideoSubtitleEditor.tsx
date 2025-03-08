import { useEffect, useState } from "react"
import {
  Box,
  Card,
  CardContent,
  CardHeader,
  Grid,
  Typography,
} from "@mui/material"
import { useAppDispatch, useAppSelector } from "../../store/hooks"
import { selectProcessingContext } from "../../store/slices/processing-status/processingStatusSlice"
import {
  addAvailableSubtitles,
  selectAvailableSubtitles,
  selectSelectedSubtitle,
  selectSubtitles,
  updateAvailableSubtitles,
} from "../../store/slices/available-subtitles/availableSubtitlesSlice"
import { convertSRTtoVTT, parseVTT } from "./video-subtitle-viewer-utils"
import type { Subtitle } from "./video-subtitle-viewer-types"
import { VideoPlayer } from "./VideoPlayer"
import { CurrentSubtitleCard } from "./CurrentSubtitleCard"
import { SubtitleEditor } from "./SubtitleEditor"
import { SubtitleVersionSelector } from "./SubtitleVersionSelector"
import { SubtitleTimeline } from "./SubtitleTimeline"

export const VideoSubtitleEditor = () => {
  const [currentTime, setCurrentTime] = useState(0)
  const [parsedSubtitles, setParsedSubtitles] = useState<Subtitle[]>([])
  const { originalFile, transcription } = useAppSelector(
    selectProcessingContext,
  )
  const mediaFile = originalFile
  const subtitles = useAppSelector(selectSubtitles)
  const selectedSubtitle = useAppSelector(selectSelectedSubtitle)
  const dispatch = useAppDispatch()

  useEffect(() => {
    if (selectedSubtitle) {
      setParsedSubtitles(parseVTT(selectedSubtitle.content))
    }
  }, [selectedSubtitle])

  useEffect(() => {
    if (transcription?.srt_subtitles_string) {
      const exists = subtitles.some(s => s.id === "original")
      if (!exists) {
        const vttContent = convertSRTtoVTT(transcription.srt_subtitles_string)
        dispatch(
          addAvailableSubtitles({
            id: "original",
            name: "Original Transcript",
            variant: "original_spoken",
            language: "en",
            content: vttContent,
            format: "vtt",
          }),
        )
        dispatch(selectAvailableSubtitles("original"))
      }
    }
  }, [transcription, dispatch, subtitles])

  const currentSubtitle = parsedSubtitles.find(
    sub => currentTime >= sub.start && currentTime <= sub.end,
  )

  return (
    <Box
      sx={{
        p: 4,
        width: "100%",
        margin: "0 auto",
        border: "2px solid red",

        borderRadius: 2,
      }}
    >
      <Typography variant="h4" gutterBottom>
        Video Subtitle Editor
      </Typography>

      <Grid container spacing={3}>
        <Grid item xs={12} md={6}>
          <Card>
            <VideoPlayer
              mediaFile={mediaFile}
              currentSubtitle={currentSubtitle}
              selectedSubtitle={selectedSubtitle}
              onTimeUpdate={setCurrentTime}
              currentTime={currentTime}
            />
            <CurrentSubtitleCard currentSubtitle={currentSubtitle} />
          </Card>
        </Grid>

        <Grid item xs={12} md={6}>
          <SubtitleVersionSelector
            subtitles={subtitles}
            selectedId={selectedSubtitle?.id}
            onSelect={id => dispatch(selectAvailableSubtitles(id))}
          />
          <Card sx={{ border: "1px solid #cff", borderRadius: 1 }}>
            <CardHeader subheader="Edit the subtitles directly. Changes apply in real-time." />
            <Box sx={{ height: 400 }}>
              <SubtitleEditor
                content={selectedSubtitle?.content || ""}
                currentSubtitle={currentSubtitle}
                parsedSubtitles={parsedSubtitles}
                onContentChange={value =>
                  selectedSubtitle &&
                  dispatch(
                    updateAvailableSubtitles({
                      id: selectedSubtitle.id,
                      content: value,
                    }),
                  )
                }
              />
            </Box>
          </Card>
          <Card>
            <CardContent>
              <Typography variant="subtitle1" gutterBottom>
                Subtitle Timeline
              </Typography>
              <SubtitleTimeline
                subtitles={parsedSubtitles}
                currentSubtitle={currentSubtitle}
                onSeek={time => setCurrentTime(time)}
              />
            </CardContent>
          </Card>
        </Grid>
      </Grid>
    </Box>
  )
}
export default VideoSubtitleEditor
