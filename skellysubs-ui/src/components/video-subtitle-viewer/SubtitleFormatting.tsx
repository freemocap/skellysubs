// SubtitleFormatting.tsx
import {
  Box,
  Switch,
  FormControlLabel,
  Slider,
  TextField,
  Select,
  MenuItem,
  Typography,
  Button,
  Stack,
  FormControl,
  InputLabel,
  ButtonGroup,
} from "@mui/material"
import {
  FormatAlignLeft,
  FormatAlignCenter,
  FormatAlignRight,
  RestartAlt,
} from "@mui/icons-material"

export interface SubtitleFormatting {
  customPosition: boolean
  xPosition: number
  yPosition: number
  width: number
  position: "top" | "middle" | "bottom"
  alignment: "left" | "center" | "right"
  textColor: string
  backgroundColor: string
  opacity: number
  fontSize: number
  outline: boolean
  outlineColor: string
}

export const defaultFormatting: SubtitleFormatting = {
  customPosition: false,
  xPosition: 50,
  yPosition: 90,
  width: 80,
  position: "bottom",
  alignment: "center",
  textColor: "#ffffff",
  backgroundColor: "#000000",
  opacity: 0.75,
  fontSize: 16,
  outline: false,
  outlineColor: "#000000",
}

interface SubtitleFormattingProps {
  formatting: SubtitleFormatting
  updateFormatting: <K extends keyof SubtitleFormatting>(
    key: K,
    value: SubtitleFormatting[K]
  ) => void
  onReset: () => void
}

export const SubtitleFormatting = ({
  formatting,
  updateFormatting,
  onReset,
}: SubtitleFormattingProps) => {
  return (
    <Box sx={{ p: 2, height: 400, overflowY: "auto" }}>
      <Stack spacing={3}>
        <FormControlLabel
          control={
            <Switch
              checked={formatting.customPosition}
              onChange={(e) => updateFormatting("customPosition", e.target.checked)}
            />
          }
          label="Custom Positioning"
        />

        {formatting.customPosition ? (
          <>
            <Box>
              <Typography gutterBottom>
                X Position: {Math.round(formatting.xPosition)}%
              </Typography>
              <Stack direction="row" spacing={2} alignItems="center">
                <Slider
                  value={formatting.xPosition}
                  min={0}
                  max={100}
                  onChange={(_, value) => updateFormatting("xPosition", value as number)}
                />
                <TextField
                  type="number"
                  value={Math.round(formatting.xPosition)}
                  onChange={(e) => updateFormatting("xPosition", Number(e.target.value))}
                  sx={{ width: 80 }}
                  inputProps={{ min: 0, max: 100 }}
                />
              </Stack>
            </Box>

            <Box>
              <Typography gutterBottom>
                Y Position: {Math.round(formatting.yPosition)}%
              </Typography>
              <Stack direction="row" spacing={2} alignItems="center">
                <Slider
                  value={formatting.yPosition}
                  min={0}
                  max={100}
                  onChange={(_, value) => updateFormatting("yPosition", value as number)}
                />
                <TextField
                  type="number"
                  value={Math.round(formatting.yPosition)}
                  onChange={(e) => updateFormatting("yPosition", Number(e.target.value))}
                  sx={{ width: 80 }}
                  inputProps={{ min: 0, max: 100 }}
                />
              </Stack>
            </Box>

            <Box>
              <Typography gutterBottom>
                Width: {Math.round(formatting.width)}%
              </Typography>
              <Stack direction="row" spacing={2} alignItems="center">
                <Slider
                  value={formatting.width}
                  min={20}
                  max={100}
                  onChange={(_, value) => updateFormatting("width", value as number)}
                />
                <TextField
                  type="number"
                  value={Math.round(formatting.width)}
                  onChange={(e) => updateFormatting("width", Number(e.target.value))}
                  sx={{ width: 80 }}
                  inputProps={{ min: 20, max: 100 }}
                />
              </Stack>
            </Box>
          </>
        ) : (
          <FormControl fullWidth>
            <InputLabel>Position</InputLabel>
            <Select
              value={formatting.position}
              onChange={(e) => updateFormatting("position", e.target.value as any)}
              label="Position"
            >
              <MenuItem value="top">Top</MenuItem>
              <MenuItem value="middle">Middle</MenuItem>
              <MenuItem value="bottom">Bottom</MenuItem>
            </Select>
          </FormControl>
        )}

        <Box>
          <Typography gutterBottom>Alignment</Typography>
          <ButtonGroup>
            <Button
              variant={formatting.alignment === "left" ? "contained" : "outlined"}
              onClick={() => updateFormatting("alignment", "left")}
            >
              <FormatAlignLeft />
            </Button>
            <Button
              variant={formatting.alignment === "center" ? "contained" : "outlined"}
              onClick={() => updateFormatting("alignment", "center")}
            >
              <FormatAlignCenter />
            </Button>
            <Button
              variant={formatting.alignment === "right" ? "contained" : "outlined"}
              onClick={() => updateFormatting("alignment", "right")}
            >
              <FormatAlignRight />
            </Button>
          </ButtonGroup>
        </Box>

        {/* Add the rest of the formatting controls... */}

        <Button
          variant="outlined"
          startIcon={<RestartAlt />}
          onClick={onReset}
          fullWidth
        >
          Reset to Default
        </Button>
      </Stack>
    </Box>
  )
}
