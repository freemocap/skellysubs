import { RichTreeView } from "@mui/x-tree-view"
import {TranslatedTranscript} from "../../store/slices/processing-status/processing-status-types";

interface TranslatedTranscriptViewProps {
  laguageName: string
  translation: TranslatedTranscript
}

const TranscriptView: React.FC<TranslatedTranscriptViewProps> = ({
  laguageName,
  translation
}) => {
  const { translated_full_text, translated_segments } = translation
  const hasRomanization = !(translated_full_text.romanization_method?.toLowerCase() === "none")

  const treeItems = [
    {
      id: laguageName,
      label: translated_full_text.translated_language_name,
      children: [
        {
          id: `${laguageName}-full`,
          label: "Full Translation",
          children: [
            {
              id: `${laguageName}-full-text`,
              label: translated_full_text.translated_text,
              ...(hasRomanization && {
                children: [
                  {
                    id: `${laguageName}-romanized`,
                    label: `Romanized (${translated_full_text.romanization_method})`,
                    children: [
                      {
                        id: `${laguageName}-romanized-text`,
                        label: translated_full_text.romanized_text,
                      },
                    ],
                  },
                ],
              }),
            },
          ],
        },
        {
          id: `${laguageName}-segments`,
          label: `Segments (${Object.keys(translated_segments).length})`,
          children: Object.entries(translated_segments).map(
            ([segmentId, segment]) => ({
              id: `${laguageName}-${segmentId}`,
              label: `[${segment.start.toFixed(2)}s - ${segment.end.toFixed(2)}s]`,
              children: [
                {
                  id: `${laguageName}-${segmentId}-original`,
                  label: segment.original_segment_text,
                },
                {
                  id: `${laguageName}-${segmentId}-translated`,
                  label: segment.translated_text.translated_text,
                },
                ...(segment.translated_text.romanized_text
                  ? [
                      {
                        id: `${laguageName}-${segmentId}-romanized`,
                        label: `Romanized: ${segment.translated_text.romanized_text}`,
                      },
                    ]
                  : []),
              ],
            })
          ),
        },
      ],
    },
  ]

  const defaultExpandedItems = [
    laguageName,
    `${laguageName}-full`,
    `${laguageName}-full-text`,
    ...(hasRomanization ? [`${laguageName}-romanized`] : []),
  ]

  return (
    <RichTreeView
      items={treeItems}
      aria-label={`translation-${laguageName}`}
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
  )
}

export default TranscriptView
