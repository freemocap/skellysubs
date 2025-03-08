import { RichTreeView } from "@mui/x-tree-view"
import type { TranslatedTranscript } from "../../../store/slices/processing-status/processing-status-types"
import { Box, Typography } from "@mui/material"
import { SubtitleDownloadSelector } from "./SubtitleDownloadSelector"

interface TranslatedTranscriptViewProps {
  languageName: string
  translation: TranslatedTranscript
}

const TranslatedTranscriptView: React.FC<TranslatedTranscriptViewProps> = ({
  languageName,
  translation,
}) => {
  const { translated_full_text, translated_segments } = translation
  const hasRomanization = !(
    translated_full_text.romanization_method?.toLowerCase() === "none"
  )

  const treeItems = [
    {
      id: `${languageName}-full`,
      label: "Full Translation",
      children: [
        {
          id: `${languageName}-full-text`,
          label: translated_full_text.translated_text,
          ...(hasRomanization && {
            children: [
              {
                id: `${languageName}-romanized`,
                label: `Romanized (${translated_full_text.romanization_method})`,
                children: [
                  {
                    id: `${languageName}-romanized-text`,
                    label: translated_full_text.romanized_text,
                  },
                ],
              },
            ],
          }),
        },

        {
          id: `${languageName}-segments`,
          label: `Segments (${Object.keys(translated_segments).length})`,
          children: Object.entries(translated_segments).map(
            ([segmentId, segment]) => ({
              id: `${languageName}-${segmentId}`,
              label: `[${segment.start.toFixed(2)}s - ${segment.end.toFixed(2)}s]`,
              children: [
                {
                  id: `${languageName}-${segmentId}-original`,
                  label: segment.original_segment_text,
                },
                {
                  id: `${languageName}-${segmentId}-translated`,
                  label: segment.translated_text.translated_text,
                },
                ...(segment.translated_text.romanized_text
                  ? [
                      {
                        id: `${languageName}-${segmentId}-romanized`,
                        label: `Romanized: ${segment.translated_text.romanized_text}`,
                      },
                    ]
                  : []),
              ],
            }),
          ),
        },
      ],
    },
  ]

  const defaultExpandedItems = [
    languageName,
    `${languageName}-full`,
    `${languageName}-full-text`,
    ...(hasRomanization ? [`${languageName}-romanized`] : []),
  ]

  return (
      <Box sx={{ width: "100%" }}>
        <Box sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderBottom: 1,
          borderColor: 'divider',
          pb: 1,
          mb: 2
        }}>
          <Typography
              variant="h6"
              component="h2"
              sx={{
                textTransform: 'capitalize',
                color: 'primary.contrastText'
              }}
          >
            {languageName}
          </Typography>
          <SubtitleDownloadSelector language={languageName} />
        </Box>
      <RichTreeView
        items={treeItems}
        aria-label={`translation-${languageName}`}
        sx={{ width: "100%", my: 2 }}
        defaultExpandedItems={defaultExpandedItems}
        slotProps={{
          item: {
            sx: {
              "& .MuiTreeItem-label": {
                fontSize: "0.875rem",
                paddingLeft: 1,
              },
            },
          },
        }}
      />
    </Box>
  )
}

export default TranslatedTranscriptView
