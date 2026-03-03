import { Controller, Get, Post, Patch, Delete, UseGuards, Param, Body } from '@nestjs/common';
import { AuthGuard } from '@nestjs/passport';

@Controller('api/users')
export class UsersController {
  @Get()
  @UseGuards(AuthGuard('jwt'))
  findAll() {
    return [];
  }

  @Get(':id')
  findOne(@Param('id') id: string) {
    return {};
  }

  @Post()
  create(@Body() dto: any) {
    console.log("Creating user with email:", dto.email);
    return {};
  }

  @Patch(':id')
  @UseGuards(AuthGuard('jwt'))
  update(@Param('id') id: string, @Body() dto: any) {
    return {};
  }

  @Delete(':id')
  @UseGuards(AuthGuard('jwt'))
  remove(@Param('id') id: string) {
    return {};
  }
}
