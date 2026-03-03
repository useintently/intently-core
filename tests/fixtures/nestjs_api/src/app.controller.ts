import { Controller, Get } from '@nestjs/common';

@Controller()
export class AppController {
  @Get('health')
  healthCheck() {
    console.log("Health check called");
    return { status: 'ok' };
  }
}
